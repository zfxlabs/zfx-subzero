use zfx_id::Id;

use crate::Result;
use crate::client;
use crate::{ice, ice::Ice};
use crate::protocol::{Request, Response};

use super::block::{self, State, Weight, BlockHash, VrfOutput};

use tracing::info;

use actix::{Actor, Handler, Context, Addr, ResponseFuture};

use rand::Rng;

use std::net::SocketAddr;
use std::path::Path;
use std::collections::HashMap;

pub struct Alpha {
    tree: sled::Db,
    ice: Addr<Ice>,
    state: State,
}

impl Alpha {
    pub fn create(path: &Path, ice: Addr<Ice>) -> Result<Self> {
	let tree = sled::open(path)?;
	Ok(Alpha { tree, ice, state: State::new() })
    }
}

impl Actor for Alpha {
    type Context = Context<Self>;

    fn started(&mut self, _ctx: &mut Context<Self>) {
	// Check for the existence of `genesis` and write to the db if it is not present.
	if !block::exists_first(&self.tree) {
	    let genesis = block::genesis();
	    let hash = block::accept_genesis(&self.tree, genesis.clone());
	    info!("accepted genesis => {:?}", hex::encode(hash));
	    self.state.apply(genesis);
	    info!("{}", self.state.format());
	} else {
	    info!("existing genesis");
	    // FIXME
	    let genesis = block::genesis();
	    self.state.apply(genesis);
	    info!("{}", self.state.format());
	}
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Message)]
#[rtype(result = "()")]
pub struct LiveNetwork {
    pub live_peers: Vec<(Id, SocketAddr)>,
}

// Queries live peers in order to determine the last accepted block
async fn query_last_accepted(peers: Vec<SocketAddr>) -> BlockHash {
    let mut i = 3;
    loop {
	info!("querying for the last accepted block");

	// TODO: Sample `k` peers if `peers.len() > k`

	// Probe `k` peers for their last accepted block ignoring errors.
	let v = client::fanout(peers.clone(), Request::GetLastAccepted).await
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

#[derive(Debug, Clone, Serialize, Deserialize, Message)]
#[rtype(result = "()")]
pub struct LiveCommittee {
    pub height: u64,
    pub total_tokens: u64,
    pub validators: Vec<(Id, u64)>,
    pub vrf_out: VrfOutput,
}

impl Handler<LiveNetwork> for Alpha {
    type Result = ResponseFuture<()>;

    fn handle(&mut self, msg: LiveNetwork, _ctx: &mut Context<Self>) -> Self::Result {
	info!("handling LiveNetwork");

	// Process the live peers in `msg`
	let mut peers = vec![];
	for (_, ip) in msg.clone().live_peers {
	    peers.push(ip);
	}

	// Read the last accepted final block (or genesis)
	let (last_hash, last_block) = block::get_last_accepted(&self.tree).unwrap();

	let ice_addr = self.ice.clone();
	let state = self.state.clone();
	Box::pin(async move {
	    let last_accepted_hash = query_last_accepted(peers).await;
	    if last_hash == last_accepted_hash {
		// Fetch the latest state snapshot up to the last hash, or apply the state
		// and persist the missing transitions to the db.
		// let (total_tokens, validators) = sync_state().await.unwrap();

		let vrf_out = last_block.vrf_out.clone();

		info!("bootstrapped => {:?}", hex::encode(last_accepted_hash));

		info!("{}", state.format());

		// If we are at the same level as the quorum then we are bootstrapped,
		// send `ice` the most up to date information concerning the peers
		// which are validating the network, such that we may determine the peers
		// `uptime` and initiate consensus.
		let () = ice_addr.send(LiveCommittee {
		    height: state.height,
		    total_tokens: state.total_tokens,
		    validators: state.validators.clone(),
		    vrf_out,
		}).await.unwrap();
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
