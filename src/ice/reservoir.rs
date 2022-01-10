use crate::zfx_id::Id;

use crate::colored::Colorize;

use super::constants::*;
use super::query::Outcome;
use super::choice::Choice;
use super::quorum::Quorum;

use tracing::info;

use rand::seq::SliceRandom;

use std::net::SocketAddr;
use std::collections::{HashMap, hash_map::Entry};

#[derive(Debug, Clone)]
pub struct Reservoir {
    quorums: HashMap<Id, Quorum>,
    decisions: HashMap<Id, (SocketAddr, Choice, usize)>,
    random_queue: Vec<(Id, (SocketAddr, Choice, usize))>,
    nbootstrapped: usize,
}

impl Reservoir {
    pub fn new() -> Reservoir {
        Reservoir {
            quorums: HashMap::new(),
            decisions: HashMap::new(),
            random_queue: vec![],
	    nbootstrapped: 0,
        }
    }

    /// Fetches the number of recorded decisions.
    pub fn len(&self) -> usize {
        self.decisions.len()
    }

    /// Fetches a decision.
    pub fn get_decision(&self, id: &Id) -> Option<(SocketAddr, Choice, usize)> {
        self.decisions.get(id).map(|(ip, choice, conviction)| {
            (ip.clone(), choice.clone(), conviction.clone())
        })
    }

    /// Fetches all decisions.
    pub fn get_decisions(&self) -> Vec<(SocketAddr, Choice, usize)> {
        self.decisions.iter()
            .map(|(_, (ip, choice, conviction))| {
                (ip.clone(), choice.clone(), conviction.clone())
            })
            .collect::<Vec<(SocketAddr, Choice, usize)>>()
    }

    /// Fetches a live peers endpoint designated by `Id` or `None` if the peer is not
    /// `Live`.
    pub fn get_live_endpoint(&self, id: &Id) -> Option<SocketAddr> {
	match self.decisions.get(id) {
	    Some((ip, choice, conviction)) => {
		if *choice == Choice::Live && *conviction >= BETA1 {
		    Some(ip.clone())
		} else {
		    None
		}
	    },
	    None => None,
	}
    }

    /// Fetches all live peers.
    pub fn get_live_peers(&self) -> Vec<(Id, SocketAddr)> {
	self.decisions.iter()
	    .fold(vec![], |mut live_peers, (id, (ip, choice, conviction))| {
		if *choice == Choice::Live && *conviction >= BETA1 {
		    live_peers.push((id.clone(), ip.clone()));
		    live_peers
		} else {
		    live_peers
		}
	    })
    }

    /// Inserts an entry into the reservoir decisions, updating the previous entry.
    pub fn insert(&mut self, peer_id: Id, ip: SocketAddr, choice: Choice, conviction: usize) {
	let v = (ip.clone(), choice.clone(), conviction);
        let _ = self.decisions.insert(peer_id.clone(), v);
    }

    /// Inserts an entry into the reservoir if none is already present.
    pub fn insert_new(&mut self, peer_id: Id, ip: SocketAddr, choice: Choice, conviction: usize) {
	let v = (ip.clone(), choice.clone(), conviction.clone());
	if let Entry::Vacant(slot) = self.decisions.entry(peer_id.clone()) {
	    slot.insert(v);
	}
    }

    /// Sets a peers choice with 0 conviction in the reservoir.
    pub fn set_choice(&mut self, peer_id: Id, new_choice: Choice) {
	if let Entry::Occupied(mut o) = self.decisions.entry(peer_id.clone()) {
	    let (_, choice, conviction) = o.get_mut();
	    *choice = new_choice;
	    *conviction = 0;
	}
    }

    /// Updates the choice for a given entry and returns whether the reservoir
    /// has obtained a bootstrap quorum (where `k` entries are decided).
    pub fn update_choice(&mut self, peer_id: Id, ip: SocketAddr, new_choice: Choice) -> bool {
	if let Entry::Occupied(mut o) = self.decisions.entry(peer_id.clone()) {
	    let (_, choice, conviction) = o.get_mut();
	    if choice.clone() != new_choice.clone() {
		// Switch to the new choice and set initial conviction to 0
		*choice = new_choice.clone();
		*conviction = 0;
	    }
	}
	self.nbootstrapped >= K
    }

    /// Regenerates the random queue based on the current decisions.
    pub fn permute(&mut self) -> bool {
        let mut rng = rand::thread_rng();
        let queue = self.decisions.iter()
            .fold(vec![], |mut v, (id, (ip, choice, conviction))| {
                // If the conviction >= BETA1 then omit the entry from the queue
                if *conviction >= BETA1 {
                    v
                } else {
                    let entry = (id.clone(), (ip.clone(), choice.clone(), conviction.clone()));
                    v.push(entry);
                    v
                }
            });
        if queue.len() > 0 {
            self.random_queue = queue;
            self.random_queue.shuffle(&mut rng);
            true
        } else {
            false
        }
    }

    /// Samples up to `k` choices for querying.
    /// Sampling for querying is done over the decisions rather than the local network
    /// connecions since otherwise querying would only involve `Live` peers.
    pub fn sample(&mut self) -> Vec<(Id, (SocketAddr, Choice, usize))> {
        if self.decisions.len() > 0 {
            // The current arity of the sample.
            let mut i = 0;
            // The current sample.
            let mut s = vec![];
            // Accumulate elements into `s` until the sample is size `K`.
            loop {
                if i >= K {
                    break;
                } else {
                    if self.random_queue.len() > 0 {
                        let entry = self.random_queue.pop().unwrap();
                        s.push(entry);
                        i += 1;
                    } else {
                        if self.permute() {
                            continue
                        } else {
                            break
                        }
                    }
                }
            }
            s.clone()
        } else {
            vec![]
        }
    }

    /// Resets the conviction of a `Faulty` decision and sets it to `Live`. This is
    /// used when a peer has responded to a query but was previously marked as `Faulty`.
    fn reset_faulty_decision(&mut self, id: Id) {
	self.decisions.entry(id.clone()).and_modify(
            |(ip, decision, conviction)| {
		match decision {
		    Choice::Faulty => {
			*decision = Choice::Live;
			*conviction = 0;
		    },
		    Choice::Live =>
			    (),
		}
	    }
	);
    }

    /// If a decision does not obtain a quorum, then the conviction in this entry is
    /// reset to 0.
    fn reset_conviction(&mut self, id: Id) {
	if let Entry::Occupied(mut o) = self.decisions.entry(id) {
	    let (_, _, c) = o.get_mut();
	    *c = 0;
	}
    }

    /// Creates a new choice and adds it to the quorums in the reservoir.
    fn process_quorum(&mut self, responder_id: Id, peer_id: Id, choice: Choice) -> Quorum {
	// Fetch the quorum corresponding to the `id`s current consensus instance.
	if let Entry::Occupied(mut o) = self.quorums.entry(peer_id.clone()) {
	    let mut quorum = o.get_mut();
            // If the responder has already influenced the outcome of this quorum
            // then skip this `responder`.
	    if quorum.contains(&responder_id) {
		return quorum.clone();
	    }
	    // Add the choice supplied by this `responder` to the quorum.
	    quorum.insert(responder_id, choice);
	    quorum.clone()
	} else {
            // If no quorum exists then include the choice of the proposed outcome
            // as a new quorum set.
            let mut quorum = Quorum::new();
            quorum.insert(responder_id, choice);
            let _ = self.quorums.insert(peer_id.clone(), quorum.clone());
	    quorum
	}
    }

    /// If a decision was made under quorum, then the entry is modified to reflect the
    /// new decision.
    fn process_decision(&mut self, id: Id, quorum: Quorum) -> bool {
        let new_decision = quorum.decide();
        if let Some(decision) = new_decision {
	    if let Entry::Occupied(mut o) = self.decisions.entry(id.clone()) {
		let (_, d, c) = o.get_mut();
		if decision.clone() != d.clone() {
		    *d = decision.clone();
		    *c = 0;
		} else {
		    *c += 1;
		    if *d == Choice::Faulty && *c >= BETA1 {
			info!("[peer] {} confirmed: {}", id.clone(), "Faulty".red());
			self.nbootstrapped -= 1;
		    } else if *d == Choice::Live && *c >= BETA1 {
			info!("[peer] {} confirmed: {}", id.clone(), "Live".green());
			self.nbootstrapped += 1;
		    }
		}
	    }
	    // Clear the quorum since it was previously decided.
            let _ = self.quorums.remove(&id);
	    // Return the `ice` bootstrap status.
	    self.nbootstrapped >= K
        } else {
	    // Clear the quorum since it was previously decided.
            let _ = self.quorums.remove(&id);
	    // Reset conviction & return the `ice` bootstrap status.
	    self.reset_conviction(id.clone());
	    false
        }
    }

    /// Processes an outcome from the peer designated by `responder_id`.
    ///
    /// Each outcome corresponds to the response to a query previously initiated by
    /// this peer and each `peer_id` mentioned in an `Outcome` corresponds to a
    /// consensus instance for a decision of `Live` or `Faulty`.
    ///
    /// TODO: match query to outcome.
    fn process_outcome(&mut self, responder_id: Id, outcome: Outcome) {
	let peer_id = outcome.peer_id.clone();
	let choice = outcome.choice.clone();

	let q = self.process_quorum(responder_id.clone(), peer_id.clone(), choice.clone());

        // If the quorum length == `k` then the quorum is complete and a decision
	// has been made.
	if q.len() >= K {
	    if self.process_decision(peer_id.clone(), q.clone()) {
		info!("{} bootstrapped", "[ice]".magenta());
            }
	}
    }
	
    /// Processes a series of outcomes which 'fill' the reservoir with choices concerning
    /// the peer designated in the outcome.
    pub fn fill(&mut self, responder_id: Id, outcomes: Vec<Outcome>) -> bool {
	// If a peer was pinged which was considered `Faulty` yet responded a set of
	// outcomes, the peers reservoir entry is reset in order to allow for
	// re-integration.
	self.reset_faulty_decision(responder_id.clone());

        for outcome in outcomes.iter() {
	    self.process_outcome(responder_id.clone(), outcome.clone());
        }

	self.nbootstrapped >= K
    }

    /// Prints the reservoir in a human readable manner.
    pub fn print(&self) -> String {
        let mut s: String = "\n".to_owned();
        // for (id, quorum) in self.quorums.iter() {
        //     s = format!("{}{:?}{}\n", s, id, quorum);
        // }
        for (_id, (ip, choice, conviction)) in self.decisions.iter() {
            s = format!(
                "{}{} {} | {:?} | {:?} {}\n",
                s,
                "⦑".cyan(),
                ip.to_string().yellow(),
                choice,
                conviction,
                "⦒".cyan(),
            );
        }
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::util;
    
    use std::net::SocketAddr;

    #[actix_rt::test]
    async fn test_insert() {
	let ip1 = "127.0.0.1:1234".parse().unwrap();
	let ip2 = "127.0.0.1:1235".parse().unwrap();
	let id1 = Id::from_ip(&ip1);
	let id2 = Id::from_ip(&ip2);

	let mut reservoir = Reservoir::new();
	assert_eq!(reservoir.len(), 0);
	reservoir.insert(id1.clone(), ip1, Choice::Live, 0);
	reservoir.insert(id2, ip2, Choice::Live, 0);
	assert_eq!(reservoir.len(), 2);

	let p1 = reservoir.get_decision(&id1).unwrap();
	let p2 = reservoir.get_decision(&id2).unwrap();
	assert_eq!(p1.clone(), (ip1.clone(), Choice::Live, 0));
	assert_eq!(p2.clone(), (ip2.clone(), Choice::Live, 0));

	let decisions = reservoir.get_decisions();
	if decisions.clone() != vec![p1.clone(), p2.clone()] {
	    assert_eq!(decisions.clone(), vec![p2.clone(), p1.clone()]);
	} else {
	    assert_eq!(decisions.clone(), vec![p1.clone(), p2.clone()]);
	}
    }

    #[actix_rt::test]
    async fn test_reset_faulty() {
	let ip1 = "127.0.0.1:1234".parse().unwrap();
	let ip2 = "127.0.0.1:1235".parse().unwrap();
	let id1 = Id::from_ip(&ip1);
	let id2 = Id::from_ip(&ip2);

	let mut reservoir = Reservoir::new();
	assert_eq!(reservoir.len(), 0);
	reservoir.insert(id1.clone(), ip1, Choice::Faulty, 0);
	reservoir.insert(id2.clone(), ip2, Choice::Faulty, 3);
	assert_eq!(reservoir.len(), 2);

	reservoir.reset_faulty_decision(id1.clone());
	let d1 = reservoir.get_decision(&id1).unwrap();
	assert_eq!(d1, (ip1.clone(), Choice::Live, 0));

	reservoir.reset_faulty_decision(id2.clone());
	let d2 = reservoir.get_decision(&id2).unwrap();
	assert_eq!(d2, (ip2.clone(), Choice::Live, 0));
    }

    #[actix_rt::test]
    async fn test_reset_conviction() {
	let ip1 = "127.0.0.1:1234".parse().unwrap();
	let ip2 = "127.0.0.1:1235".parse().unwrap();
	let id1 = Id::from_ip(&ip1);
	let id2 = Id::from_ip(&ip2);

	let mut reservoir = Reservoir::new();
	assert_eq!(reservoir.len(), 0);
	reservoir.insert(id1.clone(), ip1, Choice::Faulty, 0);
	reservoir.insert(id2.clone(), ip2, Choice::Faulty, 3);
	assert_eq!(reservoir.len(), 2);

	reservoir.reset_conviction(id1.clone());
	let d1 = reservoir.get_decision(&id1).unwrap();
	assert_eq!(d1, (ip1.clone(), Choice::Faulty, 0));

	reservoir.reset_conviction(id2.clone());
	let d2 = reservoir.get_decision(&id2).unwrap();
	assert_eq!(d2, (ip2.clone(), Choice::Faulty, 0));
    }

    #[actix_rt::test]
    async fn test_decide() {
	let ip1 = "127.0.0.1:1234".parse().unwrap();
	let ip2 = "127.0.0.1:1235".parse().unwrap();
	let id1 = Id::from_ip(&ip1);
	let id2 = Id::from_ip(&ip2);

	let mut reservoir = Reservoir::new();
	assert_eq!(reservoir.len(), 0);
	reservoir.insert(id1.clone(), ip1, Choice::Faulty, 0);
	reservoir.insert(id2.clone(), ip2, Choice::Faulty, 3);
	assert_eq!(reservoir.len(), 2);

	// A quorum of `Live` | `Faulty` should not affect the decision
	let mut q1 = Quorum::new();
	q1.insert(id1.clone(), Choice::Live);
	q1.insert(id2.clone(), Choice::Faulty);
	reservoir.process_decision(id1.clone(), q1.clone());
	let d1 = reservoir.get_decision(&id1).unwrap();
	assert_eq!(d1, (ip1.clone(), Choice::Faulty, 0));

	// A quorum of `Faulty` | `Faulty` should increase conviction
	let mut q2 = Quorum::new();
	q2.insert(id1.clone(), Choice::Faulty);
	q2.insert(id2.clone(), Choice::Faulty);
	reservoir.process_decision(id1.clone(), q2.clone());
	let d2 = reservoir.get_decision(&id1).unwrap();
	assert_eq!(d2, (ip1.clone(), Choice::Faulty, 1));

	// A quorum of `Live` | `Live` should flip the decision
	let mut q3 = Quorum::new();
	q3.insert(id1.clone(), Choice::Live);
	q3.insert(id2.clone(), Choice::Live);
	reservoir.process_decision(id1.clone(), q3.clone());
	let d3 = reservoir.get_decision(&id1).unwrap();
	assert_eq!(d3, (ip1.clone(), Choice::Live, 0));
    }
    
    #[actix_rt::test]
    async fn test_outcome() {
	let ip1 = "127.0.0.1:1234".parse().unwrap();
	let ip2 = "127.0.0.1:1235".parse().unwrap();
	let ip3 = "127.0.0.1:1236".parse().unwrap();
	let id1 = Id::from_ip(&ip1);
	let id2 = Id::from_ip(&ip2);
	let id3 = Id::from_ip(&ip3);

	let mut reservoir = Reservoir::new();
	assert_eq!(reservoir.len(), 0);
	reservoir.insert(id1.clone(), ip1, Choice::Live, 0);
	reservoir.insert(id2.clone(), ip2, Choice::Live, 0);
	reservoir.insert(id3.clone(), ip3, Choice::Live, 0);
	assert_eq!(reservoir.len(), 3);

	// `id1` voted that `id2` is `Faulty`
	reservoir.process_outcome(id1, Outcome {
	    peer_id: id2.clone(),
	    choice: Choice::Faulty,
	});
	// `id2` voted that itself is `Faulty` (byzantine)
	reservoir.process_outcome(id2, Outcome {
	    peer_id: id2.clone(),
	    choice: Choice::Faulty,
	});
    }
}
