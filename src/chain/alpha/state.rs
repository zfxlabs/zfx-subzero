use super::block::Block;
use super::tx::{Transaction, TxHash, UTXOId};
use super::{Error, Result};

use crate::colored::Colorize;

use crate::zfx_id::Id;

use tai64::Tai64;

use std::collections::HashMap;

use tracing::error;

//-- State resulting from the application of blocks

pub type Weight = f64;

#[derive(Debug, Clone)]
pub struct State {
    pub height: u64,
    pub token_supply: u64,
    pub total_stake: u64,
    pub validators: Vec<(Id, u64)>,
    pub txs: HashMap<TxHash, Transaction>,
}

//-- Apply transactions to the current state

impl State {
    pub fn new() -> State {
        State {
            height: 0,
            token_supply: 0,
            total_stake: 0,
            validators: vec![],
            txs: HashMap::default(),
        }
    }

    pub fn apply(&mut self, block: Block) -> Result<()> {
        self.height = block.height;

        // Compute the total supply
        for tx in block.txs.iter() {
            match tx.clone() {
                Transaction::CoinbaseTx(inner_tx) => {
                    // TODO: Verify tx
                    if inner_tx.inputs().len() != 0 {
                        return Err(Error::InvalidCoinbaseInputs(inner_tx.clone()));
                    }
                    if inner_tx.outputs().len() != 1 {
                        return Err(Error::InvalidCoinbaseOutputs);
                    }

                    // Coinbase transactions create new spendable outputs.
                    let tx_hash = tx.hash();
                    self.txs.insert(tx_hash, tx.clone());

                    // Coinbase transactions create new tokens.
                    self.token_supply += inner_tx.tx.sum();
                }
                // Staking transactions spend coinbase transactions as inputs, thus they do
                // not change the token supply.
                Transaction::StakeTx(inner_tx) => {
                    // TODO: Verify tx

                    // Staking transactions consume spendable outputs and create new ones.
                    // TODO: We assume 'forever' staking here, ignoring lock time.
                    for input in inner_tx.inputs().iter() {
                        let tx_hash = input.source.clone();
                        self.txs.remove(&tx_hash);
                    }
                    let tx_hash = tx.hash();
                    self.txs.insert(tx_hash, tx.clone());

                    self.validators.push((inner_tx.node_id.clone(), inner_tx.value));

                    self.total_stake += inner_tx.value;
                }
                Transaction::TransferTx(inner_tx) => {
                    // TODO: Verify tx

                    // Transfer transactions consume spendable outputs and create new ones.
                    for input in inner_tx.inputs().iter() {
                        let tx_hash = input.source.clone();
                        self.txs.remove(&tx_hash);
                    }
                    let tx_hash = tx.hash();
                    self.txs.insert(tx_hash, tx.clone());
                }
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
