use crate::zfx_id::Id;

use crate::colored::Colorize;

use crate::client;
use crate::hail::block::HailBlock;
use crate::hail::{self, Hail};
use crate::protocol::{Request, Response};
use crate::sleet::{self, Sleet};
use crate::Result;
use crate::{ice, ice::Ice};

use crate::storage::block;

use super::block::{build_genesis, Block};
use super::state::State;
use super::types::BlockHash;

use tracing::{debug, info};

use actix::{Actor, Addr, Context, Handler, ResponseFuture};

use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::Path;

pub struct Alpha {
    tree: sled::Db,
    ice: Addr<Ice>,
    sleet: Addr<Sleet>,
    hail: Addr<Hail>,
    state: State,
}

impl Alpha {
    pub fn create(
        path: &Path,
        ice: Addr<Ice>,
        sleet: Addr<Sleet>,
        hail: Addr<Hail>,
    ) -> Result<Self> {
        let tree = sled::open(path)?;
        Ok(Alpha { tree, ice, sleet, hail, state: State::new() })
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

#[derive(Debug, Clone, Serialize, Deserialize, Message)]
#[rtype(result = "()")]
pub struct LiveNetwork {
    pub self_id: Id,
    pub live_peers: Vec<(Id, SocketAddr)>,
}

// Queries live peers in order to determine the last accepted block
async fn query_last_accepted(peers: Vec<SocketAddr>) -> BlockHash {
    let mut i = 3;
    loop {
        debug!("querying for the last accepted block");

        // TODO: Sample `k` peers if `peers.len() > k`

        // Probe `k` peers for their last accepted block ignoring errors.
        let v = client::fanout(
            peers.clone(),
            Request::GetLastAccepted,
            crate::client::FIXME_UPGRADER.clone(),
        )
        .await
        .iter()
        .filter_map(|response| {
            if let Response::LastAccepted(last_accepted) = response {
                Some(last_accepted.hash.clone())
            } else {
                None
            }
        })
        .collect::<Vec<BlockHash>>();

        // If `k * alpha` peers agree to an accepted hash then return the last accepted
        // hash.
        let mut occurences: HashMap<BlockHash, usize> = HashMap::new();
        for last_accepted in v.iter() {
            if let Some(count) = occurences.get(last_accepted) {
                let count_clone = count.clone();
                occurences.insert(last_accepted.clone(), count + 1);
                if count_clone + 1 >= (ice::K as f64 * ice::ALPHA).ceil() as usize {
                    return last_accepted.clone();
                }
            } else {
                occurences.insert(last_accepted.clone(), 0);
            }
        }

        // Otherwise continue requesting the last block hash with an exponential backoff.
        let duration = tokio::time::Duration::from_millis(1000) * i;
        actix::clock::sleep(duration).await;
        i += 1;
    }
}

impl Handler<LiveNetwork> for Alpha {
    type Result = ResponseFuture<()>;

    fn handle(&mut self, msg: LiveNetwork, _ctx: &mut Context<Self>) -> Self::Result {
        debug!("handling LiveNetwork");

        let self_id = msg.self_id.clone();

        // Process the live peers in `msg`
        let mut peers = vec![];
        for (_, ip) in msg.clone().live_peers {
            peers.push(ip);
        }

        // Read the last accepted final block (or genesis)
        let (last_hash, last_block) = block::get_last_accepted(&self.tree).unwrap();

        let ice_addr = self.ice.clone();
        let sleet_addr = self.sleet.clone();
        let hail_addr = self.hail.clone();
        let state = self.state.clone();
        Box::pin(async move {
            let last_accepted_hash = query_last_accepted(peers).await;
            if last_hash == last_accepted_hash {
                // Fetch the latest state snapshot up to the last hash, or apply the state
                // and persist the missing transitions to the db.
                // let (initial_supply, validators) = sync_state().await.unwrap();

                let vrf_out = last_block.vrf_out.clone();

                info!("[{}] last_accepted = {}", "alpha".yellow(), hex::encode(last_accepted_hash));
                // info!("{}", state.format());

                //-------------------------------------------------------------------------
                // If we are at the same level as the quorum then we are bootstrapped.
                //-------------------------------------------------------------------------

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
                let last_accepted_block = HailBlock::new(None, last_block.clone());

                // Send `hail` the live committee information for querying blocks.
                let () = hail_addr
                    .send(hail::LiveCommittee {
                        last_accepted_hash,
                        last_accepted_block,
                        height: state.height,
                        self_id: self_id.clone(),
                        self_staking_capacity: committee.self_staking_capacity.clone(),
                        total_staking_capacity: state.total_staking_capacity,
                        validators: committee.hail_validators.clone(),
                        vrf_out,
                    })
                    .await
                    .unwrap();
            } else {
                info!("chain requires bootstrapping ...");
                // Apply state transitions until the last accepted hash
            }
        })
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

    fn handle(&mut self, msg: Bootstrap, ctx: &mut Context<Self>) -> Self::Result {
        // The `alpha` bootstrapping procedure fetches the ancestors of a block recursively
        // until `genesis`.

        Bootstrapped {}
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Message)]
#[rtype(result = "LastAccepted")]
pub struct GetLastAccepted;

#[derive(Debug, Clone, Serialize, Deserialize, MessageResponse)]
pub struct LastAccepted {
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

    fn handle(&mut self, msg: AcceptedBlock, ctx: &mut Context<Self>) -> Self::Result {
        info!("[{}] received accepted block", "alpha".yellow());

        // TODO
    }
}
