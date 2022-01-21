use super::cell_id::CellId;
use super::cell_unlock_script::CellUnlockScript;
use super::output_index::OutputIndex;
use super::types::*;
use super::Result;

use std::hash::Hash;

use ed25519_dalek::{Keypair, Signer};

/// A cell input (reference to a spent cell).
#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
pub struct Input {
    pub output_index: OutputIndex,
    pub unlock: CellUnlockScript,
}

impl std::fmt::Display for Input {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}", self.output_index.cell_id().unwrap())
    }
}

impl Input {
    pub fn new(keypair: &Keypair, cell_hash: CellHash, index: u8) -> Result<Self> {
        let output_index = OutputIndex::new(cell_hash.clone(), index);
        let cell_id: [u8; 32] = output_index.cell_id()?.into();
        let signature = keypair.sign(&cell_id);
        let unlock = CellUnlockScript::new(keypair.public.clone(), signature);
        Ok(Input { output_index, unlock })
    }

    pub fn cell_id(&self) -> Result<CellId> {
        self.output_index.cell_id()
    }
}
