use super::{Result, Error};
use super::block::Block;
use super::tx::{Transaction, UTXOId};

use crate::colored::Colorize;

use zfx_id::Id;

use tai64::Tai64;

use std::collections::HashSet;

use tracing::error;

//-- State resulting from the application of blocks

pub type Weight = f64;

#[derive(Debug, Clone)]
pub struct State {
    pub height: u64,
    pub token_supply: u64,
    pub validators: Vec<(Id, u64)>,
    pub utxo_ids: HashSet<UTXOId>,
}

//-- Apply transactions to the current state

impl State {
    pub fn new() -> State {
	State {
	    height: 0,
	    token_supply: 0,
	    validators: vec![],
	    utxo_ids: HashSet::new(),
	}
    }

    pub fn apply(&mut self, block: Block) -> Result<()> {
	self.height = block.height;
	
	// Compute the total supply
	for tx in block.txs.iter() {
	    match tx {
		Transaction::CoinbaseTx(tx) => {
		    if tx.inputs().len() != 0 {
			return Err(Error::InvalidCoinbaseInputs(tx.clone()));
		    }
		    if tx.outputs().len() != 1 {
			return Err(Error::InvalidCoinbaseOutputs);
		    }
		    // Coinbase transactions create new tokens.
		    self.token_supply += tx.tx.sum();
		    // Coinbase transactions create new spendable outputs.
		    let source = tx.hash().to_vec();
		    let utxo_id_bytes = vec![source, vec![0]].concat();
		    let utxo_id_encoded = bincode::serialize(&utxo_id_bytes).unwrap();
		    let utxo_id = blake3::hash(&utxo_id_encoded).as_bytes().clone();
		    self.utxo_ids.insert(utxo_id);
		},
		// Staking transactions spend coinbase transactions as inputs, thus they do
		// not change the token supply.
		Transaction::StakeTx(tx) => {
		    // Staking transactions spend outputs (which therefore are no longer
		    // spendable until the staking period ends). 
		    for input in tx.inputs().iter() {
			let utxo_id = input.utxo_id();
			if !self.utxo_ids.remove(&utxo_id.clone()) {
			    return Err(Error::InvalidUTXO(utxo_id));
			}
		    }

		    // TODO: Staking transactions may spend less than the total within an
		    // output, supplying the difference back to the staker (and producing
		    // a spendable output).
		    //
		    // NOTE: Since we assume 'forever' staking here, we ignore lock time /
		    // other concerns.
		    self.validators.push((tx.node_id.clone(), tx.tx.sum()));
		},
	    }
	}

	Ok(())
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
