use super::{Input, Inputs, Output, Outputs, PublicKeyHash, Transaction, Tx, TxHash};

use crate::colored::Colorize;

use ed25519_dalek::Keypair;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransferTx {
    /// The inputs / outputs of this transaction.
    pub tx: Tx,
}

impl std::fmt::Display for TransferTx {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let h = hex::encode(self.hash());
        let s = format!("[{}] {}\n", "tx_hash".yellow(), h);
        let s = format!("{}[{}] Transfer\n", s, "type".yellow());
        let s = format!("{}[{}] {}\n", s, "spendable".yellow(), self.tx.sum());
        write!(f, "{}", s)
    }
}

impl TransferTx {
    /// Creates a new transfer transaction. Transfer transactions are used to send tokens
    /// from the owner of `keypair` to some destination public key hash.
    pub fn new(
        keypair: &Keypair,
        tx: Transaction,
        to_address: PublicKeyHash,
        change_address: PublicKeyHash,
        value: u64,
    ) -> Self {
        let tx_hash = tx.hash();
        let inner_tx = tx.inner();
        let tx = inner_tx.spend(keypair, tx.hash(), to_address, change_address, value).unwrap();
        TransferTx { tx }
    }

    pub fn inputs(&self) -> Inputs<Input> {
        self.tx.inputs()
    }

    pub fn outputs(&self) -> Outputs<Output> {
        self.tx.outputs()
    }

    pub fn hash(&self) -> [u8; 32] {
        // Assuming a transfer is fully identified by its inputs and outputs
        self.tx.hash()
    }
}
