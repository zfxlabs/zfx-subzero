use zfx_id::Id;

use crate::colored::Colorize;
use super::block::Block;
use super::tx::Transaction;

use tai64::Tai64;

//-- State resulting from the application of blocks

pub type Weight = f64;

#[derive(Debug, Clone)]
pub struct State {
    pub height: u64,
    pub token_supply: u64,
    pub validators: Vec<(Id, u64)>,
}

//-- Apply transactions to the current state

impl State {
    pub fn new() -> State {
	State { height: 0, token_supply: 0, validators: vec![] }
    }

    pub fn apply(&mut self, block: Block) {
	self.height = block.height;
	
	// Compute the total supply
	for tx in block.txs.iter() {
	    match tx {
		Transaction::StakeTx(stake_tx) => {
		    self.token_supply += stake_tx.tx.sum();
		},
	    }
	}

	// TODO: For testing purposes we make every validator stake forever
	for tx in block.txs.iter() {
	    match tx {
		Transaction::StakeTx(stake_tx) => {
		    // if stake_tx.start_time <= Tai64::now() && stake_tx.end_time >= Tai64::now() {
		    //let w: f64 = percent_of(stake_tx.qty, self.total_tokens);
		
		    self.validators.push((stake_tx.node_id.clone(), stake_tx.tx.sum()));
		    //}
		}
	    }
	}
    }

    pub fn format(&self) -> String {
	let token_supply = format!("Σ = {:?}", self.token_supply).cyan();
	let mut s: String = format!("{}\n", token_supply);
	for (id, w) in self.validators.clone() {
	    let id_s = format!("{:?}", id).yellow();
	    let w_s = format!("{:?}", w).magenta();
	    s = format!("{} ν = {} {} | {} {}\n", s, "⦑".cyan(), id_s, w_s, "⦒".cyan());
	}
	s
    }
}
