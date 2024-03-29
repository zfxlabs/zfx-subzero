pub mod status_handler;

use crate::zfx_id::Id;

use crate::colored::Colorize;

use crate::client::{ClientRequest, ClientResponse};
use crate::hail::block::HailBlock;
use crate::hail::{self, Hail};
use crate::protocol::{Request, Response};
use crate::server::{InitRouter, Router, ValidatorSet};
use crate::sleet::{self, Sleet};
use crate::storage::block;
use crate::{ice, ice::Ice};

use super::block::{build_genesis, Block};
use super::state::State;
use super::types::{BlockHash, VrfOutput};
use super::Result;

use actix::{Actor, Addr, Arbiter, AsyncContext, Context, Handler, Recipient};
use actix::{ActorFutureExt, ResponseActFuture, WrapFuture};
use tracing::{debug, info};

use std::collections::{hash_map::Entry, HashMap, HashSet};
use std::net::SocketAddr;
use std::path::Path;

/// The actor for `alpha` chain component which
/// defines all chains known to nodes in the network and implements `Proof-of-Stake`.
///
/// Upon instantiation, builds a genesis [Block] if doesn't exist
/// in the `tree` (storage), and applies it into the `state`.
///
/// Once bootstrapped, it enables [sleet][crate::sleet] component to receive [Cell][crate::cell::Cell]s
/// and kick off consensus protocol on them.
pub struct Alpha {
    /// The client for making external requests to other nodes in the network.
    sender: Recipient<ClientRequest>,
    /// The id of the node.
    node_id: Id,
    /// The database root for storing blocks.
    tree: sled::Db,
    /// The address of the [Ice][crate::ice] actor.
    pub ice: Addr<Ice>,
    /// The address of the [Sleet][crate::sleet] actor.
    pub sleet: Addr<Sleet>,
    /// The address of the [Hail][crate::hail] actor.
    pub hail: Addr<Hail>,
    /// The address of the [Router][crate::server::Router] actor.
    router: Option<Addr<Router>>,
    /// The `alpha` chain state.
    pub state: State,
}

impl Alpha {
    /// Create new instance with opening a connection to the `tree` storage.
    ///
    /// ## Parameters
    /// * `sender` - the client for making external requests to other nodes in the network
    /// * `node_id` - [Id] of the current node
    /// * `path` - path to a database file where `tree` is stored
    /// * `ice` - the address of the [Ice][crate::ice] actor
    /// * `sleet` - the address of the [Sleet][crate::sleet] actor
    /// * `hail` - he address of the [Hail][crate::hail] actor
    pub fn create(
        sender: Recipient<ClientRequest>,
        node_id: Id,
        path: &Path,
        ice: Addr<Ice>,
        sleet: Addr<Sleet>,
        hail: Addr<Hail>,
    ) -> Result<Self> {
        let tree = sled::open(path)?;
        Ok(Alpha { sender, node_id, tree, ice, sleet, hail, router: None, state: State::new() })
    }

    /// Return a set of validators (nodes) [Id]s with staked capacity > 0.
    fn get_validator_set(&self) -> HashSet<Id> {
        self.state
            .validators
            .iter()
            .filter_map(|(id, capacity)| if *capacity > 0 { Some(id.clone()) } else { None })
            .collect()
    }
}

impl Actor for Alpha {
    type Context = Context<Self>;

    fn started(&mut self, _ctx: &mut Context<Self>) {
        // Check for the existence of `genesis` and write to the db if it is not present.
        if !block::exists_genesis(&self.tree) {
            let genesis = build_genesis().unwrap();
            let hash = block::accept_genesis(&self.tree, genesis.clone()).unwrap();
            info!("accepted genesis => {:?}", hex::encode(hash));
            let genesis_state = self.state.apply(genesis).unwrap();
            self.state = genesis_state;
            info!("{}", self.state.format());
        } else {
            let (hash, genesis) = block::get_genesis(&self.tree).unwrap();
            info!("existing genesis => {:?}", hex::encode(hash));
            let genesis_state = self.state.apply(genesis).unwrap();
            self.state = genesis_state;
            info!("{}", self.state.format());
        }
    }
}

impl Handler<InitRouter> for Alpha {
    type Result = ();

    fn handle(&mut self, InitRouter { addr }: InitRouter, ctx: &mut Context<Self>) -> Self::Result {
        self.router = Some(addr.clone());
        let validators = self.get_validator_set();
        ctx.spawn(
            async move {
                let _ = addr.send(ValidatorSet { validators }).await;
            }
            .into_actor(self),
        );
    }
}

/// A message to initiate a process of fetching the last accepted block from each of the `peers`
/// and notify the current node sending the [ReceiveLastAccepted] message,
/// if ([ice::K] * [ice::ALPHA]) - nodes agree to accept the block.
#[derive(Debug, Clone, Serialize, Deserialize, Message)]
#[rtype(result = "()")]
pub struct QueryLastAccepted {
    /// A list of nodes used for retrieving the last accepted blocks from each of them
    peers: Vec<(Id, SocketAddr)>,
}

impl Handler<QueryLastAccepted> for Alpha {
    type Result = ResponseActFuture<Self, ()>;

    fn handle(&mut self, msg: QueryLastAccepted, _ctx: &mut Context<Self>) -> Self::Result {
        // Read the last accepted final block (or genesis)
        let (_last_hash, last_block) = block::get_last_accepted(&self.tree).unwrap();

        let send_to_client = self
            .sender
            .send(ClientRequest::Fanout { peers: msg.peers, request: Request::GetLastAccepted });
        // Probe `k` peers for their last accepted block ignoring errors.
        let send_to_client = actix::fut::wrap_future::<_, Self>(send_to_client);
        let handle_response = send_to_client.map(move |result, _actor, ctx| {
            match result {
                Ok(ClientResponse::Fanout(responses)) => {
                    let v = responses
                        .iter()
                        .filter_map(|response| {
                            if let Response::LastAccepted(last_accepted) = response {
                                Some(last_accepted.hash.clone())
                            } else {
                                None
                            }
                        })
                        .collect::<Vec<BlockHash>>();

                    // If `k * alpha` peers agree to an accepted hash then return the last
                    // accepted hash.
                    let mut occurences: HashMap<BlockHash, usize> = HashMap::new();
                    let mut already_applied: HashSet<BlockHash> = HashSet::new();
                    for last_accepted in v.iter() {
                        match occurences.entry(*last_accepted) {
                            Entry::Occupied(mut o) => {
                                if !already_applied.contains(last_accepted) {
                                    let count = o.get_mut();
                                    *count += 1;
                                    if *count >= (ice::K as f64 * ice::ALPHA).ceil() as usize {
                                        ctx.notify(ReceiveLastAccepted {
                                            last_block_hash: last_accepted.clone(),
                                            last_block: last_block.clone(),
                                            last_vrf_output: last_block.vrf_out.clone(),
                                            last_accepted: last_accepted.clone(),
                                        });
                                        already_applied.insert(last_accepted.clone());
                                    }
                                }
                            }
                            Entry::Vacant(v) => {
                                let _ = v.insert(0);
                            }
                        }
                    }
                }
                // TODO: handle error
                Ok(ClientResponse::Oneshot(_)) => (),
                // TODO: handle error
                Err(_) => (),
            }
        });
        Box::pin(handle_response)
    }
}

/// A message to notify the current node about the last accepted block from another node.
///
/// This notification message is triggered from [QueryLastAccepted].
///
/// If `last_block_hash` equals to `last_accepted`, then it notifies [Sleet][crate::sleet],
/// [Ice][crate::ice] and [Hail][crate::hail] components about the updated `state` of the chain,
/// the validator set of committee, live cells and the last accepted block where applicable.
#[derive(Debug, Clone, Serialize, Deserialize, Message)]
#[rtype(result = "()")]
pub struct ReceiveLastAccepted {
    last_block_hash: BlockHash,
    last_block: Block,
    last_vrf_output: VrfOutput,
    last_accepted: BlockHash,
}

impl Handler<ReceiveLastAccepted> for Alpha {
    type Result = ();

    fn handle(&mut self, msg: ReceiveLastAccepted, _ctx: &mut Context<Self>) -> Self::Result {
        let ice_addr = self.ice.clone();
        let sleet_addr = self.sleet.clone();
        let hail_addr = self.hail.clone();
        let state = self.state.clone();
        let router = self.router.clone();
        let validators = self.get_validator_set();

        if msg.last_block_hash == msg.last_accepted {
            // Fetch the latest state snapshot up to the last hash, or apply the state
            // and persist the missing transitions to the db.
            // let (initial_supply, validators) = sync_state().await.unwrap();

            info!("[{}] last_accepted = {}", "alpha".yellow(), hex::encode(msg.last_accepted));
            // info!("{}", state.format());

            //-------------------------------------------------------------------------
            // If we are at the same level as the quorum then we are bootstrapped.
            //-------------------------------------------------------------------------

            let node_id = self.node_id.clone();

            let initialize = async move {
                // Update the router's knowledge of validators
                if let Some(addr) = router {
                    addr.send(ValidatorSet { validators }).await.unwrap();
                }
                // Send `ice` the most up to date information concerning the peers which
                // are validating the network, such that we may determine the peers
                // `uptime`.
                let committee = ice_addr
                    .send(ice::LiveCommittee {
                        total_staking_capacity: state.total_staking_capacity,
                        validators: state.validators.clone(),
                    })
                    .await
                    .unwrap();

                // Convert the states live cells to a `CellHash` mapping for `sleet` (FIXME).
                let mut map = HashMap::default();
                for (_, cell) in state.live_cells.iter() {
                    let _ = map.insert(cell.hash(), cell.clone());
                }

                // Send `sleet` the live committee information for querying transactions.
                let () = sleet_addr
                    .send(sleet::LiveCommittee {
                        validators: committee.sleet_validators.clone(),
                        live_cells: map,
                    })
                    .await
                    .unwrap();

                // Build a `HailBlock` from the last accepted block.
                let last_accepted_block = HailBlock::new(None, msg.last_block.clone());

                // Send `hail` the live committee information for querying blocks.
                let () = hail_addr
                    .send(hail::LiveCommittee {
                        last_accepted_hash: msg.last_accepted,
                        last_accepted_block,
                        height: state.height,
                        self_id: node_id.clone(),
                        self_staking_capacity: committee.self_staking_capacity.clone(),
                        total_staking_capacity: state.total_staking_capacity,
                        validators: committee.hail_validators.clone(),
                        vrf_out: msg.last_vrf_output,
                    })
                    .await
                    .unwrap();
            };

            let arbiter = Arbiter::new();
            arbiter.spawn(initialize);
        } else {
            info!("chain requires bootstrapping ...");
            // Apply state transitions until the last accepted hash
        }
    }
}

/// A message used by [Ice][crate::ice] to notify `alpha` about a change of at least
/// one node in the network if it's status changed from [Faulty][crate::ice::Choice::Faulty] to [Live][crate::ice::Choice::Live].
///
/// It will notify `alpha` with [QueryLastAccepted] to get last accepted blocks from these `peers`.
#[derive(Debug, Clone, Serialize, Deserialize, Message)]
#[rtype(result = "()")]
pub struct LiveNetwork {
    /// Id of the current node
    pub self_id: Id,
    /// a list of peers which status changed to [Live][crate::ice::Choice::Live]
    pub live_peers: Vec<(Id, SocketAddr)>,
}

impl Handler<LiveNetwork> for Alpha {
    type Result = ();

    fn handle(&mut self, msg: LiveNetwork, ctx: &mut Context<Self>) -> Self::Result {
        debug!("handling LiveNetwork");

        // Process the live peers in `msg`
        let mut peers = vec![];
        for (id, ip) in msg.clone().live_peers {
            peers.push((id, ip));
        }

        // Initiate the process of fetching the last accepted block
        ctx.notify(QueryLastAccepted { peers })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Message)]
#[rtype(result = "()")]
pub struct FaultyNetwork;

impl Handler<FaultyNetwork> for Alpha {
    type Result = ();

    fn handle(&mut self, _msg: FaultyNetwork, _ctx: &mut Context<Self>) -> Self::Result {
        info!(": handling FaultyNetwork -> Halt FSM");
        ()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Message)]
#[rtype(result = "Bootstrapped")]
pub struct Bootstrap;

#[derive(Debug, Clone, Serialize, Deserialize, MessageResponse)]
pub struct Bootstrapped;

impl Handler<Bootstrap> for Alpha {
    type Result = Bootstrapped;

    fn handle(&mut self, _msg: Bootstrap, _ctx: &mut Context<Self>) -> Self::Result {
        // The `alpha` bootstrapping procedure fetches the ancestors of a block recursively
        // until `genesis`.

        Bootstrapped {}
    }
}

/// A message to request the last accepted block in the `tree` of the current node.
#[derive(Debug, Clone, Serialize, Deserialize, Message)]
#[rtype(result = "LastAccepted")]
pub struct GetLastAccepted;

/// Response to [GetLastAccepted] with a block hash.
#[derive(Debug, Clone, Serialize, Deserialize, MessageResponse)]
pub struct LastAccepted {
    /// Has of the last accepted block of the current node.
    hash: BlockHash,
}

impl Handler<GetLastAccepted> for Alpha {
    type Result = LastAccepted;

    fn handle(&mut self, _msg: GetLastAccepted, _ctx: &mut Context<Self>) -> Self::Result {
        let last_accepted_hash = block::get_last_accepted_hash(&self.tree).unwrap();
        LastAccepted { hash: last_accepted_hash }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Message)]
#[rtype(result = "Ancestors")]
pub struct GetAncestors;

#[derive(Debug, Clone, Serialize, Deserialize, MessageResponse)]
pub struct Ancestors;

impl Handler<GetAncestors> for Alpha {
    type Result = Ancestors;

    fn handle(&mut self, _msg: GetAncestors, _ctx: &mut Context<Self>) -> Self::Result {
        Ancestors {}
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Message)]
#[rtype(result = "()")]
pub struct AcceptedBlock {
    pub block: Block,
}

impl Handler<AcceptedBlock> for Alpha {
    type Result = ();

    fn handle(&mut self, _msg: AcceptedBlock, _ctx: &mut Context<Self>) -> Self::Result {
        info!("[{}] received accepted block", "alpha".yellow());

        // TODO
    }
}
