use crate::zfx_id::Id;

use super::choice::Choice;
use super::constants::*;

use std::collections::HashSet;

// A quorum is a list of choices which can be decided when `i == k`

#[derive(Debug, Clone)]
pub struct Quorum {
    pub ids: HashSet<Id>,
    pub choices: Vec<Choice>,
}

impl std::fmt::Display for Quorum {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(fmt, "Q {:?} {:?}", self.ids, self.choices)
    }
}

impl Quorum {
    pub fn new() -> Quorum {
        Quorum { ids: HashSet::new(), choices: vec![] }
    }

    pub fn len(&self) -> usize {
        self.ids.len()
    }

    pub fn contains(&self, id: &Id) -> bool {
        self.ids.contains(id)
    }

    pub fn insert(&mut self, observer_id: Id, choice: Choice) {
        if !self.ids.contains(&observer_id) {
            let _ = self.ids.insert(observer_id);
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
