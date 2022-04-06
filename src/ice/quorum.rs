use crate::zfx_id::Id;

use super::choice::Choice;
use super::constants::*;

use std::collections::{HashMap, HashSet};

/// A quorum is a list of choices which can be decided when `i == k`
#[derive(Debug, Clone)]
pub struct Quorum {
    pub choices: HashMap<Id, Choice>,
}

impl std::fmt::Display for Quorum {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(fmt, "Q {:?}", self.choices)
    }
}

impl Quorum {
    pub fn new() -> Quorum {
        Quorum { choices: HashMap::new() }
    }

    pub fn len(&self) -> usize {
        self.choices.len()
    }

    pub fn contains(&self, id: &Id) -> bool {
        self.choices.contains_key(id)
    }

    pub fn insert(&mut self, observer_id: Id, choice: Choice) {
        self.choices.insert(observer_id, choice);
    }

    /// Make a decision whether the quorum
    /// has more than (K * ALPHA) Live or Faulty choices.
    ///
    /// Return None if decision threshold didn't pass (K * ALPHA)
    pub fn decide(&self) -> Option<Choice> {
        let mut n_live = 0;
        let mut n_faulty = 0;
        for (_, choice) in self.choices.iter() {
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
