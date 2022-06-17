use crate::p2p::linear_backoff::Execute;
use crate::p2p::peer_meta::PeerMetadata;
use crate::p2p::prelude::*;
use crate::p2p::response_handler::ResponseHandler;
use crate::p2p::{sender, sender::Sender};

use crate::message::{Ping, PingAck};

use crate::protocol::network::{NetworkRequest, NetworkResponse};

use crate::alpha::{self, Alpha};
use crate::cell::types::Capacity;
use crate::colored::Colorize;
use crate::util;
use crate::{Error, Result};

use super::choice::Choice;
use super::constants::*;
use super::query::{Outcome, Query};
use super::reservoir::Reservoir;
use super::sampleable_map::SampleableMap;

use std::collections::HashMap;

pub struct Ice {
    /// The connection upgrader.
    upgrader: Arc<dyn Upgrader>,
    /// The metadata pertaining to this peer.
    self_peer: PeerMetadata,
    /// The reservoir which `ice` samples queries from.
    reservoir: Reservoir,
    /// The latest known validator map of the primary protocol.
    validator_map: SampleableMap<PeerMetadata, Capacity>,
    /// The `ice` main loop protocol period.
    protocol_period: Duration,
    /// Whether `ice` is bootstrapped or not.
    bootstrapped: bool,
}

impl Ice {
    pub fn new(
        upgrader: Arc<dyn Upgrader>,
        self_peer: PeerMetadata,
        validators: Vec<(PeerMetadata, Capacity)>,
    ) -> Self {
        let mut validator_map = SampleableMap::new();
        for (peer_meta, capacity) in validators.iter().cloned() {
            let _ = validator_map.insert(peer_meta, capacity);
        }
        Ice {
            upgrader,
            self_peer,
            reservoir: Reservoir::new(),
            validator_map,
            protocol_period: Duration::from_secs(6),
            bootstrapped: false,
        }
    }

    pub fn sample_queries(&mut self, peer_meta: PeerMetadata) -> Vec<Query> {
        // If the peer metadata is not in the reservoir, insert it
        let () = self.reservoir.insert_new(peer_meta.clone(), Choice::Live, 0);

        let mut queries = vec![];
        if self.reservoir.len() > 0 {
            let sample = self.reservoir.sample();
            for (peer_meta, (choice, _conviction)) in sample.iter() {
                queries.push(Query { peer_meta: peer_meta.clone(), choice: choice.clone() });
            }
        } else {
            error!("! reservoir uninitialised");
        }
        queries
    }
}

impl Actor for Ice {
    type Context = Context<Self>;
}

impl Handler<Execute> for Ice {
    type Result = ResponseActFuture<Self, bool>;

    fn handle(&mut self, msg: Execute, ctx: &mut Context<Self>) -> Self::Result {
        ctx.notify(PrintReservoir);

        // Sample a peer at random from the validator map
        let sample = self.validator_map.sample(1);
        // Take the first sample (there is only one)
        let (peer_meta, _capacity) = sample[0].clone();
        // Sample the pending queries in the reservoir
        let queries = self.sample_queries(peer_meta.clone());
        // Ping the peer
        let ping_handler = PingHandler::new(ctx.address());
        let sender_address = Sender::new(self.upgrader.clone(), ping_handler).start();
        let request = NetworkRequest::Ping(Ping::new(self.self_peer.clone(), queries));
        // Wrap the future and box
        let send_fut = sender::send::<NetworkRequest, NetworkResponse>(
            sender_address,
            peer_meta.clone(),
            request,
            self.protocol_period.clone(),
        );
        let send_wrapped = actix::fut::wrap_future::<_, Self>(send_fut);
        Box::pin(
            // TODO: Returning false here implies `ice` never shuts down
            send_wrapped.map(move |response, _, ctx| match response {
                Ok(()) => false,
                Err(_) => {
                    ctx.notify(ReceivePingFailure { peer_meta });
                    false
                }
            }),
        )
    }
}

/// Processes a query into an `Outcome`.
fn process_query(reservoir: &mut Reservoir, self_peer: PeerMetadata, query: Query) -> Outcome {
    let peer_meta = query.peer_meta.clone();
    let choice = query.choice.clone();

    // If the queried `id` is the same as the `self_id` then the outcome should
    // always be `Live`.
    if peer_meta.clone() == self_peer.clone() {
        return Outcome::new(peer_meta.clone(), Choice::Live);
    }

    match reservoir.get_decision(&peer_meta) {
        Some((_, choice, _)) => Outcome::new(peer_meta, choice),
        None => match choice.clone() {
            // If we have not made a decision pertaining to this peer a
            // priori and the choice is `Live`, then our choice becomes
            // `Live`.
            Choice::Live => {
                let () = reservoir.set_choice(peer_meta.clone(), Choice::Live);
                Outcome::new(peer_meta, choice)
            }
            // If we have not made a decision pertaining to this peer a
            // priori and the choice is `Faulty`, then our choice becomes
            // `Faulty` if an indirect ping request fails.
            Choice::Faulty => {
                let () = reservoir.set_choice(peer_meta.clone(), Choice::Faulty);
                Outcome::new(peer_meta, choice)
            }
        },
    }
}

impl Handler<Ping> for Ice {
    type Result = PingAck;

    fn handle(&mut self, msg: Ping, _ctx: &mut Context<Self>) -> Self::Result {
        // Processes incoming queries from the server
        let mut outcomes = vec![];
        for query in msg.queries.iter().cloned() {
            info!("<- {:?}", query.clone());
            let outcome = process_query(&mut self.reservoir, self.self_peer.clone(), query);
            outcomes.push(outcome);
        }
        // Send the outcomes as response
        PingAck { peer_meta: self.self_peer.clone(), outcomes }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Message)]
#[rtype(result = "()")]
pub struct ReceivePingSuccess {
    ping_ack: PingAck,
}

impl Handler<ReceivePingSuccess> for Ice {
    type Result = ();

    // The peer responded successfully
    fn handle(&mut self, msg: ReceivePingSuccess, _ctx: &mut Context<Self>) -> Self::Result {
        let ping_ack = msg.ping_ack.clone();
        if self.reservoir.fill(ping_ack.peer_meta, ping_ack.outcomes) {
            // When `fill` returns true `ice` is bootstrapped
        } else {
            // When `fill` returns false `ice` is not (or no longer) bootstrapped
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Message)]
#[rtype(result = "bool")]
pub struct ReceivePingFailure {
    peer_meta: PeerMetadata,
}

impl Handler<ReceivePingFailure> for Ice {
    type Result = bool;

    // The peer did not respond or responded erroneously
    fn handle(&mut self, msg: ReceivePingFailure, _ctx: &mut Context<Self>) -> Self::Result {
        // If updating the choice to `Faulty` reverts `ice` to a non-bootstrapped state,
        // communicate this to the `alpha` chain.
        if !self.reservoir.update_choice(msg.peer_meta, Choice::Faulty) {
            if self.bootstrapped {
                return true;
            }
        }
        return false;
    }
}

pub struct PingHandler {
    addr: Addr<Ice>,
}

impl PingHandler {
    pub fn new(addr: Addr<Ice>) -> Arc<dyn ResponseHandler<NetworkResponse>> {
        Arc::new(PingHandler { addr })
    }
}

// A `PingAck` is responded when a `Ping` request is made to a per. The `PingAck` sends the peer
// response to `ice` such that the outcome of the queries may be processed.
impl ResponseHandler<NetworkResponse> for PingHandler {
    fn handle_response(
        &self,
        response: NetworkResponse,
    ) -> Pin<Box<dyn Future<Output = Result<()>>>> {
        let addr = self.addr.clone();
        match response {
            NetworkResponse::PingAck(ping_ack) => Box::pin(async move {
                let () = addr.send(ReceivePingSuccess { ping_ack }).await.unwrap();
                Ok(())
            }),
            _ => Box::pin(async { Ok(()) }),
        }
    }
}

// When the `Alpha` network becomes `Live` and bootstraps the chain state, `Ice` is informed
// via the `alpha::LiveCommittee` message, which provides the validator set for the current
// height.

// #[derive(Debug, Clone, Serialize, Deserialize, Message)]
// #[rtype(result = "Committee")]
// pub struct LiveCommittee {
//     pub total_staking_capacity: u64,
//     pub validators: Vec<(Id, u64)>,
// }

// #[derive(Debug, Clone, Serialize, Deserialize, MessageResponse)]
// pub struct Committee {
//     pub self_staking_capacity: u64,
//     pub sleet_validators: HashMap<Id, (SocketAddr, f64)>,
//     pub hail_validators: HashMap<Id, (SocketAddr, u64)>,
// }

// impl Handler<LiveCommittee> for Ice {
//     type Result = Committee;

//     // We augment the list of validators from the `LiveCommittee` with the validator
//     // endpoints, such that subsequent consensus algorithms can probe the peers.
//     fn handle(&mut self, msg: LiveCommittee, _ctx: &mut Context<Self>) -> Self::Result {
//         info!("[{}] received live committee", "ice".to_owned().magenta());
//         let mut sleet_validators = HashMap::default();
//         let mut hail_validators = HashMap::default();
//         info!("[{}] live committee size = {}", "ice".magenta(), msg.validators.len());
//         let mut self_staking_capacity = None;
//         for (id, amount) in msg.validators.iter() {
//             if id.clone() == self.id {
//                 self_staking_capacity = Some(*amount);
//             } else {
//                 match self.reservoir.get_live_endpoint(id) {
//                     Some(ip) => {
//                         let w = util::percent_of(*amount, msg.total_staking_capacity);
//                         let _ = sleet_validators.insert(id.clone(), (ip.clone(), w));
//                         let _ = hail_validators.insert(id.clone(), (ip.clone(), *amount));
//                     }
//                     None => (),
//                 }
//             }
//         }
//         let self_staking_capacity = if let Some(self_staking_capacity) = self_staking_capacity {
//             self_staking_capacity
//         } else {
//             panic!("insufficient stake");
//         };
//         Committee { self_staking_capacity, sleet_validators, hail_validators }
//     }
// }

#[derive(Debug, Clone, Serialize, Deserialize, Message)]
#[rtype(result = "LivePeers")]
pub struct GetLivePeers;

#[derive(Debug, Clone, Serialize, Deserialize, MessageResponse)]
pub struct LivePeers {
    pub live_peers: Vec<PeerMetadata>,
}

impl Handler<GetLivePeers> for Ice {
    type Result = LivePeers;

    // The peer did not respond or responded erroneously
    fn handle(&mut self, _msg: GetLivePeers, _ctx: &mut Context<Self>) -> Self::Result {
        LivePeers { live_peers: self.reservoir.get_live_peers() }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Message)]
#[rtype(result = "Status")]
pub struct CheckStatus;

#[derive(Debug, Clone, Serialize, Deserialize, MessageResponse)]
pub struct Status {
    pub bootstrapped: bool,
}

impl Handler<CheckStatus> for Ice {
    type Result = Status;

    fn handle(&mut self, _msg: CheckStatus, _ctx: &mut Context<Self>) -> Self::Result {
        Status { bootstrapped: self.bootstrapped }
    }
}

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
