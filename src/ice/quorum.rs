use crate::p2p::id::Id;
use crate::p2p::peer_meta::PeerMetadata;

use super::choice::Choice;
use super::constants::*;

use std::collections::HashSet;

// A quorum is a list of choices which can be decided when `i == k`

#[derive(Debug, Clone)]
pub struct Quorum {
    pub peers: HashSet<PeerMetadata>,
    pub choices: Vec<Choice>,
}

impl std::fmt::Display for Quorum {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(fmt, "Q {:?} {:?}", self.peers, self.choices)
    }
}

impl Quorum {
    pub fn new() -> Quorum {
        Quorum { peers: HashSet::new(), choices: vec![] }
    }

    pub fn len(&self) -> usize {
        self.peers.len()
    }

    pub fn contains(&self, peer_meta: &PeerMetadata) -> bool {
        self.peers.contains(peer_meta)
    }

    pub fn insert(&mut self, observer: PeerMetadata, choice: Choice) {
        if !self.peers.contains(&observer) {
            let _ = self.peers.insert(observer);
            let _ = self.choices.push(choice);
        }
    }

    pub fn decide(&self) -> Option<Choice> {
        let mut n_live = 0;
        let mut n_faulty = 0;
        for choice in self.choices.iter() {
            match choice {
                Choice::Live => {
                    n_live += 1;
                }
                Choice::Faulty => {
                    n_faulty += 1;
                }
            }
        }
        if n_live > (K as f64 * ALPHA).ceil() as usize {
            return Some(Choice::Live);
        }
        if n_faulty > (K as f64 * ALPHA).ceil() as usize {
            return Some(Choice::Faulty);
        }
        None
    }
}
