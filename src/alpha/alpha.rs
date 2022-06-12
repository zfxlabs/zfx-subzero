//! The `alpha` primary chain protocol actor. This actor is responsible for handling requests for
//! the primary graph state as well as requests for the latest validator set post bootstrap.

use crate::cell::types::Capacity;
use crate::cell::CellId;
use crate::p2p::id::Id;

use actix::{Actor, Addr, Arbiter, AsyncContext, Context, Handler, Recipient};
use actix::{ActorFutureExt, ResponseActFuture, WrapFuture};
use tracing::{debug, info};

use super::genesis;
use super::state::State;
use super::{Error, Result};

pub struct Alpha {
    /// The `id` of the primary chain.
    chain_id: Id,
    /// The path to the database of the primary chain.
    chain_db_path: String,
    /// The database of the primary chain.
    chain_db: Option<sled::Db>,
    /// The chain state.
    chain_state: Option<State>,
    /// The last cell id in the chain state.
    last_cell_id: Option<CellId>,
}

impl Alpha {
    pub fn new(chain_id: Id, chain_db_path: String) -> Self {
        Alpha { chain_id, chain_db_path, chain_db: None, chain_state: None, last_cell_id: None }
    }

    /// Opens the database of the primary chain for reading and reads all known cells.
    /// TODO: optimise
    pub fn init(&mut self) -> Result<()> {
        // TODO: Make this a streaming solution for constant space memory overhead (?)

        // Opens the primary chains database at `path`.
        let db = sled::open(&self.chain_db_path)?;
        // Read all existing cells or create the genesis cells
        let (cell_ids, cells) = genesis::read_or_create_cells(&db).unwrap();
        // Save the last cell hash for comparison with other peers
        let last_cell_id = cells[cells.len() - 1].id();
        // Apply the cells to a new genesis state
        let mut chain_state = State::new();
        for (i, cell) in cells.iter().cloned().enumerate() {
            info!("[{:?}] applying cell: {:?}", i, cell);
            chain_state.apply_cell(cell).unwrap();
        }

        info!("(alpha) initialised `chain_db`");
        self.chain_db = Some(db);
        self.chain_state = Some(chain_state);
        // TODO: Save the last state hash for comparison with other peers (?)
        // let last_state_hash = genesis_state.hash();
        info!("(alpha) last_cell_id = {:?}", last_cell_id.clone());
        self.last_cell_id = Some(last_cell_id);

        Ok(())
    }
}

impl Actor for Alpha {
    type Context = Context<Self>;

    fn started(&mut self, _ctx: &mut Context<Self>) {
        self.init().unwrap()
    }
}

/// The `alpha` actor receives `LastCellId` requests from other peers and the bootstrap actor. `alpha`
/// will responds the latest cell known in the state.
#[derive(Debug, Clone, Message)]
#[rtype(result = "Result<CellId>")]
pub struct LastCellId;

impl Handler<LastCellId> for Alpha {
    type Result = Result<CellId>;

    fn handle(&mut self, msg: LastCellId, ctx: &mut Context<Self>) -> Self::Result {
        match &self.last_cell_id {
            Some(last_cell_id) => Ok(last_cell_id.clone()),
            None => Err(Error::AlphaUninitialised),
        }
    }
}

/// The `alpha` actor receives `Ancestors` requests from other peers and the bootstrap actor. `alpha`
/// will respond all of the ancestors which relate to the `CellId`.
#[derive(Debug, Clone, Message)]
#[rtype(result = "Result<()>")]
pub struct Ancestors {
    pub cell_id: CellId,
}

impl Handler<Ancestors> for Alpha {
    type Result = Result<()>;

    fn handle(&mut self, msg: Ancestors, ctx: &mut Context<Self>) -> Self::Result {
        match &self.chain_db {
            Some(db) => Err(Error::AlphaDbUninitialised),
            None => Err(Error::AlphaDbUninitialised),
        }
    }
}

/// The `alpha` actor receives `ValidatorSet` requests from the bootstrapper. `alpha` will respond
/// the validator set corresponding to the `LastCellId` provided.
#[derive(Debug, Clone, Message)]
#[rtype(result = "Result<Vec<(Id, Capacity)>>")]
pub struct ValidatorSet {
    pub cell_id: CellId,
}

impl Handler<ValidatorSet> for Alpha {
    type Result = Result<Vec<(Id, Capacity)>>;

    fn handle(&mut self, msg: ValidatorSet, ctx: &mut Context<Self>) -> Self::Result {
        match &self.last_cell_id {
            Some(last_cell_id) => match &self.chain_state {
                Some(chain_state) => Ok(chain_state.validators.clone()),
                None => Err(Error::AlphaInvalidChainState),
            },
            None => Err(Error::AlphaOutOfSync),
        }
    }
}
