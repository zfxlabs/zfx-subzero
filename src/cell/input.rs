use super::cell_id::CellId;
use super::cell_unlock_script::CellUnlockScript;
use super::output_index::OutputIndex;
use super::types::*;
use super::Result;

use std::hash::Hash;

use ed25519_dalek::{Keypair, Signer};

/// Part of [Cell] structure which represents a reference to a spent [Output] of a cell
/// with a signature of serialized [CellId].
#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
pub struct Input {
    /// Reference to an [Output] within a [Cell], based on its position (index) in
    /// an [Outputs] list.
    pub output_index: OutputIndex,
    /// _not in use at the moment, as transactions are not signed_
    pub unlock: CellUnlockScript,
}

impl std::fmt::Display for Input {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}", self.output_index.cell_id().unwrap())
    }
}

impl Input {
    /// Create a new instance of Input.
    ///
    /// ## Parameters:
    /// * `keypair` - account's keypair for signing serialized `cell_hash` and `index`,
    /// and assigning it to `unlock` property of Input.
    /// * `cell_hash` - hash of a [Cell] being spent.
    /// * `index` - position of [Output] in the list of [Outputs] in [Cell].
    pub fn new(keypair: &Keypair, cell_hash: CellHash, index: u8) -> Result<Self> {
        let output_index = OutputIndex::new(cell_hash.clone(), index);
        let cell_id: [u8; 32] = output_index.cell_id()?.into();
        let signature = keypair.sign(&cell_id);
        let unlock = CellUnlockScript::new(keypair.public.clone(), signature);
        Ok(Input { output_index, unlock })
    }

    /// Returns a cell id from [OutputIndex::cell_id].
    pub fn cell_id(&self) -> Result<CellId> {
        self.output_index.cell_id()
    }
}
