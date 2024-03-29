use crate::zfx_id::Id;

use crate::alpha::{self, Alpha};
use crate::client::{ClientRequest, ClientResponse};
use crate::colored::Colorize;
use crate::protocol::{Request, Response};
use crate::util;
use crate::view::{self, View};
use crate::{Error, Result};

use super::choice::Choice;
use super::constants::*;
use super::dissemination;
use super::dissemination::{Gossip, GossipQuery};
use super::query::{Outcome, Query};
use super::reservoir::Reservoir;

use tracing::{debug, error, info};

use actix::{Actor, Addr, Context, Handler, Recipient};
use actix::{ActorFutureExt, ResponseActFuture};

use std::collections::HashMap;
use std::net::SocketAddr;

use actix::WrapFuture;

/// The main actor
///
/// `Ice` actor is responsible for dissemination and gossiping of peers. It holds information about the bootstrapping
/// status of current node, as well as a list of connected nodes in the network ([Reservoir]) with their statuses
/// (ex. [Live][Choice::Live] or [Faulty][Choice::Faulty] in order to identify whether there is sufficient number of
/// connected nodes (quorum) in the network prior to bootstrapping the [alpha][crate::alpha] chain (making it
/// available to accept requests).
pub struct Ice {
    /// The client used to make external requests.
    sender: Recipient<ClientRequest>,
    /// Id of this node
    pub id: Id,
    /// IP address of this node
    pub ip: SocketAddr,
    /// The [`Reservoir`][super::Reservoir] to sample from
    reservoir: Reservoir,
    /// Whether `ice` is bootstrapped and ready
    bootstrapped: bool,
    /// Address of the [`DisseminationComponent`][super::dissemination::DisseminationComponent] to
    /// pull gossip messages from
    dc_recipient: Recipient<GossipQuery>,
}

impl Ice {
    pub fn new(
        sender: Recipient<ClientRequest>,
        id: Id,
        ip: SocketAddr,
        reservoir: Reservoir,
        dc_recipient: Recipient<GossipQuery>,
    ) -> Self {
        Ice { sender, id, ip, reservoir, bootstrapped: false, dc_recipient }
    }
}

impl Actor for Ice {
    type Context = Context<Self>;

    fn started(&mut self, _ctx: &mut Context<Self>) {
        debug!(": started");
    }
}

/// Network ping message
///
/// It carries gossip messages, and [queries][super::Query] about other nodes' liveness.
#[derive(Debug, Clone, Serialize, Deserialize, Message)]
#[rtype(result = "Ack")]
pub struct Ping {
    pub id: Id,
    pub queries: Vec<Query>,
    pub rumours: Vec<Gossip>,
}

/// Network response to a [`Ping`]
///
/// Contains the results for the [queries][super::Query] in the `Ping` message.
#[derive(Debug, Clone, Serialize, Deserialize, MessageResponse)]
pub struct Ack {
    pub id: Id,
    pub outcomes: Vec<Outcome>,
}
/// Processes a query into an `Outcome`c.
fn process_query(reservoir: &mut Reservoir, self_id: Id, query: Query) -> Outcome {
    let peer_id = query.peer_id.clone();
    let choice = query.choice.clone();

    // If the queried `id` is the same as the `self_id` then the outcome should
    // always be `Live`.
    if peer_id.clone() == self_id.clone() {
        return Outcome::new(peer_id.clone(), Choice::Live);
    }

    match reservoir.get_decision(&peer_id) {
        Some((_, choice, _)) => Outcome::new(peer_id, choice),
        None => match choice.clone() {
            // If we have not made a decision pertaining to this peer a
            // priori and the choice is `Live`, then our choice becomes
            // `Live`.
            Choice::Live => {
                let () = reservoir.set_choice(peer_id, Choice::Live);
                Outcome::new(peer_id, choice)
            }
            // If we have not made a decision pertaining to this peer a
            // priori and the choice is `Faulty`, then our choice becomes
            // `Faulty` if an indirect ping request fails.
            Choice::Faulty => {
                let () = reservoir.set_choice(peer_id, Choice::Faulty);
                Outcome::new(peer_id, choice)
            }
        },
    }
}

impl Handler<Ping> for Ice {
    type Result = Ack;

    fn handle(&mut self, msg: Ping, _ctx: &mut Context<Self>) -> Self::Result {
        // Processes incoming queries from the server
        let mut outcomes = vec![];
        for query in msg.queries.iter().cloned() {
            info!("<- {:?}", query);
            let outcome = process_query(&mut self.reservoir, self.id.clone(), query);
            outcomes.push(outcome);
        }
        // Send the outcomes as response
        Ack { id: self.id, outcomes }
    }
}

/// Instruct [`Ice`] to start bootstrapping
#[derive(Debug, Clone, Serialize, Deserialize, Message)]
#[rtype(result = "Bootstrapped")]
pub struct Bootstrap {
    /// Trusted bootstrap peers (from the node's configuration)
    pub peers: Vec<(Id, SocketAddr)>,
}

/// Response to [`Bootstrap`], whether [`Ice`] finished bootstapping
#[derive(Debug, Clone, Serialize, Deserialize, MessageResponse)]
pub struct Bootstrapped(pub bool);

impl Handler<Bootstrap> for Ice {
    type Result = Bootstrapped;

    fn handle(&mut self, msg: Bootstrap, _ctx: &mut Context<Self>) -> Self::Result {
        debug!("received bootstrap peers {:?}", msg.peers);
        for (id, ip) in msg.peers.iter() {
            self.reservoir.insert(id.clone(), ip.clone(), Choice::Live, 0);
        }
        Bootstrapped(true)
    }
}

/// Actor message to sample a set of peers for querying their status
#[derive(Debug, Clone, Serialize, Deserialize, Message)]
#[rtype(result = "Queries")]
pub struct SampleQueries {
    /// The `view` based sample - this is used to add new entries to the reservoir.
    sample: (Id, SocketAddr),
}

/// Message containing the queries sampled
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
            for (id, (ip, choice, _conviction)) in sample.iter() {
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
struct PingSuccess {
    ack: Ack,
}

#[derive(Debug, Clone, Serialize, Deserialize, MessageResponse)]
struct Switch {
    flipped: bool,
    bootstrapped: bool,
}

impl Handler<PingSuccess> for Ice {
    type Result = Switch;

    // The peer responded successfully
    fn handle(&mut self, msg: PingSuccess, _ctx: &mut Context<Self>) -> Self::Result {
        let ack = msg.ack.clone();
        let (is_bootstrapped, flipped) = self.reservoir.fill(ack.id, ack.outcomes);
        if is_bootstrapped {
            if self.bootstrapped {
                Switch { flipped, bootstrapped: true }
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
struct PingFailure {
    id: Id,
    ip: SocketAddr,
}

impl Handler<PingFailure> for Ice {
    type Result = bool;

    // The peer did not respond or responded erroneously
    fn handle(&mut self, msg: PingFailure, _ctx: &mut Context<Self>) -> Self::Result {
        // If updating the choice to `Faulty` reverts `ice` to a non-bootstrapped state,
        // communicate this to the `alpha` chain.
        if !self.reservoir.update_choice(msg.id, Choice::Faulty) {
            if self.bootstrapped {
                return true;
            }
        }
        return false;
    }
}

/// Actor message to request a list of live peers
#[derive(Debug, Clone, Serialize, Deserialize, Message)]
#[rtype(result = "LivePeers")]
pub struct GetLivePeers;

/// Message containing the list of live peers
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

/// Message from [`alpha`][crate::alpha] containing the set of known validators
///
/// When the `Alpha` network becomes `Live` and bootstraps the chain state, `Ice` is informed
/// via the `alpha::LiveCommittee` message, which provides the validator set for the current
/// height.
#[derive(Debug, Clone, Serialize, Deserialize, Message)]
#[rtype(result = "Committee")]
pub struct LiveCommittee {
    pub total_staking_capacity: u64,
    pub validators: Vec<(Id, u64)>,
}

/// Reply message to [`LiveCommittee`], containing the validators that are live
#[derive(Debug, Clone, Serialize, Deserialize, MessageResponse)]
pub struct Committee {
    pub self_staking_capacity: u64,
    pub sleet_validators: HashMap<Id, (SocketAddr, f64)>,
    pub hail_validators: HashMap<Id, (SocketAddr, u64)>,
}

impl Handler<LiveCommittee> for Ice {
    type Result = Committee;

    // We augment the list of validators from the `LiveCommittee` with the validator
    // endpoints, such that subsequent consensus algorithms can probe the peers.
    fn handle(&mut self, msg: LiveCommittee, _ctx: &mut Context<Self>) -> Self::Result {
        info!("[{}] received live committee", "ice".to_owned().magenta());

        let mut hail_validators = HashMap::default();
        info!("[{}] live committee size = {}", "ice".magenta(), msg.validators.len());
        let mut self_staking_capacity = None;
        let mut total_staking_capacity = 0; // total stake of the node + other Live validators of the node
        for (id, amount) in msg.validators.iter() {
            if *id == self.id {
                total_staking_capacity += *amount;
                self_staking_capacity = Some(*amount);
            } else if let Some(ip) = self.reservoir.get_live_endpoint(id) {
                total_staking_capacity += *amount;
                let _ = hail_validators.insert(id.clone(), (ip.clone(), *amount));
            }
        }

        let sleet_validators = msg
            .validators
            .iter()
            .filter(|(id, _)| *id != self.id)
            .filter_map(|(id, amount)| {
                // if validator is Live then re-calculate it's weight based on
                // total stake of other Live validators
                if let Some(ip) = self.reservoir.get_live_endpoint(id) {
                    let w = util::percent_of(*amount, total_staking_capacity);
                    return Some((id.clone(), ((ip.clone(), w))));
                }
                return None;
            })
            .collect();

        let self_staking_capacity = if let Some(self_staking_capacity) = self_staking_capacity {
            self_staking_capacity
        } else {
            panic!("insufficient stake");
        };
        Committee { self_staking_capacity, sleet_validators, hail_validators }
    }
}

/// Message to print the state of the reservoir
#[derive(Debug, Clone, Serialize, Deserialize, Message)]
#[rtype(result = "()")]
pub struct PrintReservoir;

impl Handler<PrintReservoir> for Ice {
    type Result = ();

    // The peer did not respond or responded erroneously
    fn handle(&mut self, _msg: PrintReservoir, _ctx: &mut Context<Self>) -> Self::Result {
        info!("{}", self.reservoir.print());
    }
}

/// Actor message to query the reservoir's size
#[derive(Debug, Clone, Serialize, Deserialize, Message)]
#[rtype(result = "usize")]
pub struct ReservoirSize;

impl Handler<ReservoirSize> for Ice {
    type Result = usize;

    fn handle(&mut self, _msg: ReservoirSize, _ctx: &mut Context<Self>) -> Self::Result {
        self.reservoir.len()
    }
}

/// Actor message to instruct [`Ice`] to ping a peer
#[derive(Debug, Clone, Serialize, Deserialize, Message)]
#[rtype(result = "Result<Ack>")]
pub struct DoPing {
    self_id: Id,
    id: Id,
    ip: SocketAddr,
    queries: Vec<Query>,
    network_size: usize,
}

impl Handler<DoPing> for Ice {
    type Result = ResponseActFuture<Self, Result<Ack>>;

    fn handle(&mut self, msg: DoPing, _ctx: &mut Context<Self>) -> Self::Result {
        let dc = self.dc_recipient.clone();
        let sender = self.sender.clone();
        Box::pin(
            async move {
                let rumours = dissemination::pull_rumours(dc, msg.network_size).await;
                sender
                    .send(ClientRequest::Oneshot {
                        id: msg.id,
                        ip: msg.ip,
                        request: Request::Ping(Ping {
                            id: msg.self_id,
                            queries: msg.queries,
                            rumours: rumours,
                        }),
                    })
                    .await
            }
            .into_actor(self)
            .map(move |result, _actor, _ctx| {
                match result {
                    Ok(ClientResponse::Oneshot(res)) => {
                        match res {
                            // Success -> Ack
                            Some(Response::Ack(ack)) => Ok(ack.clone()),
                            // Failure (byzantine)
                            Some(_) => Err(Error::Byzantine),
                            // Failure (crash)
                            None => Err(Error::Crash),
                        }
                    }
                    Ok(_) => Err(Error::InvalidResponse),
                    Err(e) => Err(Error::Actix(e)),
                }
            }),
        )
    }
}

/// Actor message to check the status of [`Ice`]
#[derive(Debug, Clone, Serialize, Deserialize, Message)]
#[rtype(result = "Status")]
pub struct CheckStatus;

/// Reply for [`CheckStatus`], containing the status of [`Ice`]
#[derive(Debug, Clone, Serialize, Deserialize, MessageResponse)]
pub struct Status {
    pub bootstrapped: bool,
    pub peers: Vec<(Id, SocketAddr, Choice)>,
}

impl Handler<CheckStatus> for Ice {
    type Result = Status;

    fn handle(&mut self, _msg: CheckStatus, _ctx: &mut Context<Self>) -> Self::Result {
        let mut validators: Vec<(Id, SocketAddr, Choice)> = vec![];
        for (id, addr, choice, _) in self.reservoir.get_decisions() {
            validators.push((id, addr, choice));
        }

        Status { bootstrapped: self.bootstrapped, peers: validators }
    }
}

async fn send_ping_success(self_id: Id, ice: Addr<Ice>, alpha: Addr<Alpha>, ack: Ack) {
    let switch = ice.send(PingSuccess { ack: ack.clone() }).await.unwrap();
    if switch.flipped {
        // If flipped from `LiveNetwork` to `FaultyNetwork`, alert the `Alpha` chain.
        if !switch.bootstrapped {
            alpha.send(alpha::FaultyNetwork).await.unwrap();
        } else {
            // Otherwise alert the `Alpha` chain of a `LiveNetwork`.
            let LivePeers { live_peers } = ice.send(GetLivePeers {}).await.unwrap();
            alpha.send(alpha::LiveNetwork { self_id, live_peers }).await.unwrap();
        }
    }
}

async fn send_ping_failure(ice: Addr<Ice>, alpha: Addr<Alpha>, id: Id, ip: SocketAddr) {
    let flipped = ice.send(PingFailure { id: id.clone(), ip: ip.clone() }).await.unwrap();
    // If flipped from `LiveNetwork` to `FaultyNetwork`, alert the `Alpha` chain.
    if flipped {
        alpha.send(alpha::FaultyNetwork).await.unwrap();
    }
}

/// Run the protocol in rounds
///
/// This function drives the `Ice` component in a loop (see [PROTOCOL_PERIOD]).
/// It samples peers to query and handles the results.
///
pub async fn run(self_id: Id, ice: Addr<Ice>, view: Addr<View>, alpha: Addr<Alpha>) {
    loop {
        let () = ice.send(PrintReservoir).await.unwrap();
        let network_size = ice.send(ReservoirSize).await.unwrap();

        // Sample a random peer from the view
        let view::SampleResult { sample } =
            view.send(view::SampleK { k: ping_size(network_size) }).await.unwrap();

        for (id, ip) in sample.iter().cloned() {
            // Sample up to `k` peers from the reservoir and collect ping queries
            let Queries { queries } =
                ice.send(SampleQueries { sample: (id.clone(), ip.clone()) }).await.unwrap();

            // Ping the designated peer

            match ice
                .send(DoPing { self_id, id: id.clone(), ip: ip.clone(), queries, network_size })
                .await
                .unwrap()
            {
                Ok(ack) => {
                    send_ping_success(self_id.clone(), ice.clone(), alpha.clone(), ack.clone())
                        .await
                }
                Err(_) => {
                    send_ping_failure(ice.clone(), alpha.clone(), id.clone(), ip.clone()).await
                }
            }
        }

        // Sleep for the protocol period duration.
        actix::clock::sleep(PROTOCOL_PERIOD).await;
    }
}

/// Determine the number of peers to ping (cater for small testnets)
fn ping_size(network_size: usize) -> usize {
    std::cmp::min(network_size, PING_MAX_SIZE)
}
