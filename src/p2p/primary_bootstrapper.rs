//! Bootstrapper responsible for bootstrapping the primary network and chain.
//!
//! The `PrimaryBootstrapper` is used to initialise the network and the primary chain in order to
//! instantiate the initial validator set required for subsequent network bootstraps. Subsequent
//! bootstrappers are expected to use trusted validator sets derived from the primary network state.

use super::prelude::*;

use super::peer_bootstrapper::ReceivePeerSet;

use crate::alpha::{genesis, state::State};

use std::collections::hash_map::Entry;
use std::collections::{HashMap, HashSet};
use std::path::Path;

pub struct PrimaryBootstrapper {
    /// The `id` of the primary chain.
    chain_id: Id,
    /// A trusted set of bootstrap peers for bootstrapping the chain.
    bootstrap_peers: HashSet<PeerMetadata>,
    /// The number of peers required to bootstrap the chain.
    bootstrap_peer_lim: usize,
    /// The path to the database of the primary chain.
    chain_db_path: String,
    /// The database of the primary chain.
    chain_db: Option<sled::Db>,
}

impl PrimaryBootstrapper {
    pub fn new(chain_id: Id, bootstrap_peer_lim: usize, chain_db_path: String) -> Self {
	PrimaryBootstrapper { chain_id, bootstrap_peers: HashSet::default(), bootstrap_peer_lim, chain_db_path, chain_db: None }
    }

    /// Opens the database of the primary chain for reading.
    pub fn open_db(&mut self) -> Result<()> {
	// Opens the primary chains database at `path`.
	let primary_chain_db = sled::open(&self.chain_db_path)?;
	info!("initialised primary `chain_db`");
	self.chain_db = Some(primary_chain_db);
	Ok(())
    }

    /// Inserts a new bootstrap peer and returns `Some(_)` when the bootstrap peer limit has been
    /// reached, otherwise `None` is returned.
    pub fn insert_bootstrap_peer(&mut self, peer_meta: PeerMetadata) -> Option<HashSet<PeerMetadata>> {
	if self.bootstrap_peers.len() >= self.bootstrap_peer_lim {
	    return Some(self.bootstrap_peers.clone());
	} else {
	    if let true = self.bootstrap_peers.insert(peer_meta) {
		if self.bootstrap_peers.len() >= self.bootstrap_peer_lim {
		    Some(self.bootstrap_peers.clone())
		} else {
		    None
		}
	    } else {
		None
	    }
	}
    }
}

impl Actor for PrimaryBootstrapper {
    type Context = Context<Self>;
}

#[derive(Debug, Clone, Message)]
#[rtype(result = "()")]
pub struct ReceivePrimaryBootstrap {
    pub chain: Id,
    pub peers: HashSet<PeerMetadata>,
}

// pub struct ReceiveSync {
//     pub last_cell_hash: Hash,
//     pub state: State,
// }

impl Handler<ReceivePeerSet> for PrimaryBootstrapper {
    type Result = ();

    fn handle(&mut self, msg: ReceivePeerSet, ctx: &mut Context<Self>) -> Self::Result {
	if let Some(db) = self.chain_db.clone() {
	    // Collects the peers which support the primary chain and tries to insert them into the
	    // primary bootstrappers
	    let mut primary_peers = HashSet::new();
	    for peer in msg.peer_set.iter().cloned() {
		if peer.chains.contains(&self.chain_id) {
		    primary_peers.insert(peer.clone());
		    match self.insert_bootstrap_peer(peer.clone()) {
			// If the primary chain has enough peers, start bootstrapping the primary
			// chain
			Some(peers) => {
			    // TODO: Make this a streaming solution for constant space memory overhead

			    // Read all existing cells or create the genesis cells
			    let (cell_ids, cells) = genesis::read_or_create_cells(&db).unwrap();
			    // Save the last cell hash for comparison with other peers
			    let last_cell_hash = cells[cells.len()-1].hash();
			    // Apply the cells to a new genesis state
			    let mut genesis_state = State::new();
			    for (i, cell) in cells.iter().cloned().enumerate() {
				info!("[{:?}] applying cell: {:?}", i, cell);
				genesis_state.apply_cell(cell).unwrap();
			    }
			    // TODO: Save the last state hash for comparison with other peers (?)
			    // let last_state_hash = genesis_state.hash();
			    info!("last_cell_hash = {:?}", last_cell_hash);
			    
			    // Synchronise the chain state according to the trusted peers
			    
			    // The synchronised cells (latest cells) are all accepted in `sleet`.

			    return;
			},
			None =>
			    continue,
		    }
		}
	    }
	} else {
	    error!("error: database unopened, skipping primary bootstrapper")
	}

			    // Apply state transitions of primary network cells
			    
			    // loop:
			    
			    // Obtain the network quorums last accepted hash, in order to sync
			    
			    // Request missing ancestry between the quorum hash and the local hash
			    
			    // Find the distance between the network quorum hash and the locally known last
			    // accepted hash. If the distance is close to 0 end the loop.
			    
			    // end-loop
			    
			    // At this point we have a valid validator set for the primary network encoded
			    // within the primary networks state, since all cells have been applied up to
			    // the latest set and the network quorum resolved to a distance of 0.
			    
			    // `ice` is initialised with the initial validator set so that the liveness
			    // of participants can begin to be evaluated. Once `ice` has sufficient `Live`
			    // participants, `sleet` is bootstrapped for the primary chain so that new
			    // cells can be received.
    }
}
