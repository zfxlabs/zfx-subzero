use zfx_id::Id;
use zfx_sortition::sortition;

use crate::{Result, Error};
use crate::client;
use crate::view::{self, View};
use crate::chain::alpha::{self, Alpha};
use crate::protocol::{Request, Response};
use super::constants::*;
use super::query::{Query, Outcome};
use super::choice::Choice;
use super::reservoir::Reservoir;

use tracing::{debug, info, error};

use actix::prelude::*;
use actix::{Actor, Addr, Context};

use std::net::SocketAddr;

use rand::Rng;

pub struct Ice {
    pub id: Id,
    pub ip: SocketAddr,
    reservoir: Reservoir,
    bootstrapped: bool,
}

impl Ice {
    pub fn new(id: Id, ip: SocketAddr, reservoir: Reservoir) -> Self {
	Ice { id, ip, reservoir, bootstrapped: false }
    }
}

impl Actor for Ice {
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut Context<Self>) {
	debug!(": started");
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Message)]
#[rtype(result = "Ack")]
pub struct Ping {
    pub id: Id,
    pub queries: Vec<Query>,
}

#[derive(Debug, Clone, Serialize, Deserialize, MessageResponse)]
pub struct Ack {
    pub id: Id,
    pub outcomes: Vec<Outcome>,
}

/// Processes a query into an `Outcome`c.
fn process_query(reservoir: &mut Reservoir, self_id: Id, query: Query) -> Outcome {
    let peer_id = query.peer_id.clone();
    let peer_ip = query.peer_ip.clone();
    let choice = query.choice.clone();

    // If the queried `id` is the same as the `self_id` then the outcome should
    // always be `Live`.
    if peer_id.clone() == self_id.clone() {
	return Outcome::new(peer_id.clone(), Choice::Live);
    }

    match reservoir.get_decision(&peer_id) {
	Some((_, choice, _)) =>
	    Outcome::new(peer_id, choice),
	None =>
	    match choice.clone() {
		// If we have not made a decision pertaining to this peer a
		// priori and the choice is `Live`, then our choice becomes
		// `Live`.
		Choice::Live => {
		    let () = reservoir.set_choice(peer_id.clone(), Choice::Live);
		    Outcome::new(peer_id, choice)
		},
		// If we have not made a decision pertaining to this peer a
		// priori and the choice is `Faulty`, then our choice becomes
		// `Faulty` if an indirect ping request fails.
		Choice::Faulty => {
		    let () = reservoir.set_choice(peer_id.clone(), Choice::Faulty);
		    Outcome::new(peer_id, choice)
		},
	    }
    }
}

impl Handler<Ping> for Ice {
    type Result = Ack;

    fn handle(&mut self, msg: Ping, _ctx: &mut Context<Self>) -> Self::Result {
	// Processes incoming queries from the server
	let mut outcomes = vec![];
	for query in msg.queries.iter().cloned() {
	    info!("received {:?}", query.clone());
	    let outcome = process_query(&mut self.reservoir, self.id.clone(), query);
	    outcomes.push(outcome);
	}
	// Send the outcomes as response
	Ack { id: self.id, outcomes }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Message)]
#[rtype(result = "Bootstrapped")]
pub struct Bootstrap {
    pub peers: Vec<(Id, SocketAddr)>,
}

#[derive(Debug, Clone, Serialize, Deserialize, MessageResponse)]
pub struct Bootstrapped;

impl Handler<Bootstrap> for Ice {
    type Result = Bootstrapped;

    fn handle(&mut self, msg: Bootstrap, _ctx: &mut Context<Self>) -> Self::Result {
	info!("received bootstrap peers {:?}", msg.peers);
	for (id, ip) in msg.peers.iter() {
	    self.reservoir.insert(id.clone(), ip.clone(), Choice::Live, 0);
	}
	Bootstrapped {}
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Message)]
#[rtype(result = "Queries")]
pub struct SampleQueries {
    /// The `view` based sample - this is used to add new entries to the reservoir.
    sample: (Id, SocketAddr),
}

#[derive(Debug, Clone, Serialize, Deserialize, MessageResponse)]
pub struct Queries {
    queries: Vec<Query>,
}

impl Handler<SampleQueries> for Ice {
    type Result = Queries;

    fn handle(&mut self, msg: SampleQueries, _ctx: &mut Context<Self>) -> Self::Result {
	let (id, ip) = msg.sample.clone();

	// If the ip address is not in the reservoir, insert it
	let () = self.reservoir.insert_new(id.clone(), ip.clone(), Choice::Live, 0);

	let mut queries = vec![];
	if self.reservoir.len() > 0 {
	    let sample = self.reservoir.sample();
	    for (id, (ip, choice, conviction)) in sample.iter() {
		queries.push(Query {
		    peer_id: id.clone(),
		    peer_ip: ip.clone(),
		    choice: choice.clone(),
		});
	    }
	} else {
	    error!("! reservoir uninitialised");
	}
	Queries { queries }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Message)]
#[rtype(result = "Switch")]
pub struct PingSuccess {
    ack: Ack,
}

#[derive(Debug, Clone, Serialize, Deserialize, MessageResponse)]
pub struct Switch {
    flipped: bool,
    bootstrapped: bool,
}

impl Handler<PingSuccess> for Ice {
    type Result = Switch;

    // The peer responded successfully
    fn handle(&mut self, msg: PingSuccess, _ctx: &mut Context<Self>) -> Self::Result {
	let ack = msg.ack.clone();
	if self.reservoir.fill(ack.id, ack.outcomes) {
	    if self.bootstrapped {
		Switch { flipped: false, bootstrapped: true }
	    } else {
		self.bootstrapped = true;
		Switch { flipped: true, bootstrapped: true }
	    }
	} else {
	    if !self.bootstrapped {
		Switch { flipped: false, bootstrapped: false }
	    } else {
		self.bootstrapped = false;
		Switch { flipped: true, bootstrapped: false }
	    }
	}
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Message)]
#[rtype(result = "bool")]
pub struct PingFailure {
    id: Id,
    ip: SocketAddr,
}

impl Handler<PingFailure> for Ice {
    type Result = bool;

    // The peer did not respond or responded erroneously
    fn handle(&mut self, msg: PingFailure, _ctx: &mut Context<Self>) -> Self::Result {
	// If updating the choice to `Faulty` reverts `ice` to a non-bootstrapped state,
	// communicate this to the `alpha` chain.
	if !self.reservoir.update_choice(msg.id, msg.ip, Choice::Faulty) {
	    if self.bootstrapped {
		return true;
	    }
	}
	return false;
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Message)]
#[rtype(result = "LivePeers")]
pub struct GetLivePeers;

#[derive(Debug, Clone, Serialize, Deserialize, MessageResponse)]
pub struct LivePeers {
    pub live_peers: Vec<(Id, SocketAddr)>,
}

impl Handler<GetLivePeers> for Ice {
    type Result = LivePeers;

    // The peer did not respond or responded erroneously
    fn handle(&mut self, _msg: GetLivePeers, _ctx: &mut Context<Self>) -> Self::Result {
	LivePeers { live_peers: self.reservoir.get_live_peers() }
    }
}

// When the `Alpha` network becomes `Live` and bootstraps the chain state, `Ice` is informed
// via the `alpha::LiveCommittee` message, which provides the validator set for the current
// height.

impl Handler<alpha::LiveCommittee> for Ice {
    type Result = ();

    // The peer did not respond or responded erroneously
    fn handle(&mut self, msg: alpha::LiveCommittee, _ctx: &mut Context<Self>) -> Self::Result {
	info!("received live committee for height = {:?}", msg.height);
	let self_id = self.id.clone();
	let expected_size = (msg.validators.len() as f64).sqrt().ceil();
	info!("expected_size = {:?}", expected_size);
	for (id, amount) in msg.validators {
	    let vrf_h = vec![id.as_bytes(), &msg.vrf_out].concat();
	    let vrf_h = blake3::hash(&vrf_h).as_bytes().clone();
	    let w = sortition::select(amount, msg.total_tokens, expected_size, &vrf_h);
	    info!("{:?} obtained w = {:?}", id, w);
	    if id == self_id {
		if w > 0 {
		    info!("this node is a block producer");
		} else {
		    info!("this node is not a block producer");
		}
	    }
	}
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Message)]
#[rtype(result = "()")]
pub struct PrintReservoir;

impl Handler<PrintReservoir> for Ice {
    type Result = ();

    // The peer did not respond or responded erroneously
    fn handle(&mut self, msg: PrintReservoir, ctx: &mut Context<Self>) -> Self::Result {
	info!("{}", self.reservoir.print());
    }
}

pub async fn ping(self_id: Id, ip: SocketAddr, queries: Vec<Query>) -> Result<Ack> {
    match client::oneshot(ip.clone(), Request::Ping(Ping { id: self_id, queries })).await {
	// Success -> Ack
	Ok(Some(Response::Ack(ack))) =>
	    Ok(ack.clone()),
	// Failure (byzantine)
	Ok(Some(_)) =>
	    Err(Error::Byzantine),
	// Failure (crash)
	Ok(None) =>
	    Err(Error::Crash),
	// Failure (crash)
	Err(err) =>
	    Err(Error::Crash),
    }
}

pub async fn send_ping_success(ice: Addr<Ice>, alpha: Addr<Alpha>, ack: Ack) {
    let switch = ice.send(PingSuccess { ack: ack.clone() }).await.unwrap();
    if switch.flipped {
	// If flipped from `LiveNetwork` to `FaultyNetwork`, alert the `Alpha` chain.
	if !switch.bootstrapped {
	    alpha.send(alpha::FaultyNetwork).await.unwrap();
	} else {
	    // Otherwise alert the `Alpha` chain of a `LiveNetwork`.
	    let LivePeers { live_peers } = ice.send(GetLivePeers{}).await.unwrap();
	    alpha.send(alpha::LiveNetwork { live_peers }).await.unwrap();
	}
    }
}

pub async fn send_ping_failure(ice: Addr<Ice>, alpha: Addr<Alpha>, id: Id, ip: SocketAddr) {
    let flipped = ice.send(PingFailure {
	id: id.clone(), ip: ip.clone(),
    }).await.unwrap();
    // If flipped from `LiveNetwork` to `FaultyNetwork`, alert the `Alpha` chain.
    if flipped {
	alpha.send(alpha::FaultyNetwork).await.unwrap();
    }
}

pub async fn run(self_id: Id, ice: Addr<Ice>, view: Addr<View>, alpha: Addr<Alpha>) {
    loop {
	let () = ice.send(PrintReservoir).await.unwrap();

	// Sample a random peer from the view
	let view::SampleResult { sample } = view.send(view::SampleOne).await.unwrap();

	for (id, ip) in sample.iter().cloned() {
	    // Sample up to `k` peers from the reservoir and collect ping queries
	    let Queries { queries } = ice.send(SampleQueries {
		sample: (id.clone(), ip.clone()),
	    }).await.unwrap();

	    // Ping the designated peer
	    match ping(self_id, ip.clone(), queries).await {
		Ok(ack) =>
		    send_ping_success(ice.clone(), alpha.clone(), ack.clone())
		    .await,
		Err(_) =>
		    send_ping_failure(ice.clone(), alpha.clone(), id.clone(), ip.clone())
		    .await,
	    }
	}

	// Sleep for the protocol period duration.
        actix::clock::sleep(PROTOCOL_PERIOD).await;
    }
}
