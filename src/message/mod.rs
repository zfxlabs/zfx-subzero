//! Protocol message definitions.
mod version;
mod last_cell_id;

pub use version::{Version, VersionAck, CURRENT_VERSION};
pub use last_cell_id::{LastCellId, LastCellIdAck};
