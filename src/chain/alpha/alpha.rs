use zfx_id::Id;

use crate::Result;
use crate::client;
use crate::util;
use crate::{ice, ice::Ice};
use crate::chain::alpha::InitialStaker;
use crate::sleet::{self, Sleet};
use crate::hail::{self, Hail};
use crate::protocol::{Request, Response};

use super::block::{self, BlockHash, VrfOutput};
use super::state::{State, Weight};

use tracing::info;

use actix::{Actor, Handler, Context, Addr, ResponseFuture};

use rand::Rng;

use std::net::SocketAddr;
use std::path::Path;
use std::collections::HashMap;

pub struct Alpha {
    tree: sled::Db,
    ice: Addr<Ice>,
    sleet: Addr<Sleet>,
    hail: Addr<Hail>,
    state: State,
}

impl Alpha {
    pub fn create(path: &Path, ice: Addr<Ice>, sleet: Addr<Sleet>, hail: Addr<Hail>) -> Result<Self> {
	let tree = sled::open(path)?;
	Ok(Alpha { tree, ice, sleet, hail, state: State::new() })
    }
}

impl Actor for Alpha {
    type Context = Context<Self>;

    fn started(&mut self, _ctx: &mut Context<Self>) {
	let stakers = vec![
	    InitialStaker::from_hex(
		"ad7f2ee3958a7f3fa2c84931770f5773ef7694fdd0bb217d90f29a94199c9d7307ca3851515c89344639fe6a4077923068d1d7fc6106701213c61d34ef8e9416".to_owned(),
		util::id_from_ip(&"127.0.0.1:1234".parse().unwrap()),
		1000,
	    ),
	    InitialStaker::from_hex(
		"5a353c630d3faf8e2d333a0983c1c71d5e9b6aed8f4959578fbeb3d3f3172886393b576de0ac1fe86a4dd416cf032543ac1bd066eb82585f779f6ce21237c0cd".to_owned(),
		util::id_from_ip(&"127.0.0.1:1235".parse().unwrap()),
		1000,
	    ),
	    InitialStaker::from_hex(
		"6f4b736b9a6894858a81696d9c96cbdacf3d49099d212213f5abce33da18716f067f8a2b9aeb602cd4163291ebbf39e0e024634f3be19bde4c490465d9095a6b".to_owned(),
		util::id_from_ip(&"127.0.0.1:1236".parse().unwrap()),
		1000,
	    ),
	];
	// Check for the existence of `genesis` and write to the db if it is not present.
	if !block::exists_first(&self.tree) {
	    let genesis = block::genesis(stakers);
	    let hash = block::accept_genesis(&self.tree, genesis.clone());
	    info!("accepted genesis => {:?}", hex::encode(hash));
	    self.state.apply(genesis).unwrap();
	    info!("{}", self.state.format());
	} else {
	    let (hash, genesis) = block::get_genesis(&self.tree).unwrap();
	    info!("existing genesis => {:?}", hex::encode(hash));
	    self.state.apply(genesis).unwrap();
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

impl Handler<LiveNetwork> for Alpha {
    type Result = ResponseFuture<()>;

    fn handle(&mut self, msg: LiveNetwork, _ctx: &mut Context<Self>) -> Self::Result {
	info!("handling LiveNetwork");

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

		info!("bootstrapped => {:?}", hex::encode(last_accepted_hash));

		info!("{}", state.format());

		//-------------------------------------------------------------------------
		// If we are at the same level as the quorum then we are bootstrapped.
		//-------------------------------------------------------------------------

		// Send `ice` the most up to date information concerning the peers
		// which are validating the network, such that we may determine the peers
		// `uptime`.
		let () = ice_addr.send(ice::LiveCommittee {
		    validators: state.validators.clone(),
		}).await.unwrap();

		// Send `sleet` the live committee information for querying transactions.
		let () = sleet_addr.send(sleet::LiveCommittee {
		    validators: state.validators.clone(),
		    initial_supply: state.token_supply,
		    utxo_ids: state.utxo_ids.clone(),
		}).await.unwrap();

		// Send `hail` the live committee information for querying blocks.
		let () = hail_addr.send(hail::LiveCommittee {
                    self_id: self_id.clone(),
		    height: state.height,
		    initial_supply: state.token_supply,
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
