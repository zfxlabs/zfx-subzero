use zfx_id::Id;

use crate::colored::Colorize;
use super::block::Block;

use tai64::Tai64;

//-- State resulting from the application of blocks

pub type Weight = f64;

#[derive(Debug, Clone)]
pub struct State {
    pub height: u64,
    pub total_tokens: u64,
    pub validators: Vec<(Id, u64)>,
}

//-- Apply transactions to the current state

impl State {
    pub fn new() -> State {
	State { height: 0, total_tokens: 0, validators: vec![] }
    }

    pub fn apply(&mut self, block: Block) {
	self.height = block.height;
	
	for stake_tx in block.txs.iter() {
	    self.total_tokens += stake_tx.qty;
	}
	// TODO: For testing purposes we make every validator stake forever
	for stake_tx in block.txs.iter() {
	    // if stake_tx.start_time <= Tai64::now() && stake_tx.end_time >= Tai64::now() {
	    //let w: f64 = percent_of(stake_tx.qty, self.total_tokens);
	    self.validators.push((stake_tx.node_id.clone(), stake_tx.qty));
	    //}
	}
    }

    pub fn format(&self) -> String {
	let total_tokens = format!("Σ = {:?}", self.total_tokens).cyan();
	let mut s: String = format!("{}\n", total_tokens);
	for (id, w) in self.validators.clone() {
	    let id_s = format!("{:?}", id).yellow();
	    let w_s = format!("{:?}", w).magenta();
	    s = format!("{} ν = {} {} | {} {}\n", s, "⦑".cyan(), id_s, w_s, "⦒".cyan());
	}
	s
    }
}
