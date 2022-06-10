use crate::cell::types::CellHash;
use crate::cell::Cell;
use crate::sleet::Sleet;
use crate::storage::tx as tx_storage;
use actix::{Context, Handler};

/// A request structure to get a cell by its hash.
/// If found, the requested cell is returned from in-memory live-cells which were accepted by consensus (sleet-component).
#[derive(Debug, Clone, Serialize, Deserialize, Message)]
#[rtype(result = "CellAck")]
pub struct GetCell {
    pub cell_hash: CellHash,
}

#[derive(Debug, Clone, Serialize, Deserialize, MessageResponse)]
pub struct CellAck {
    pub cell: Option<Cell>,
}

impl Handler<GetCell> for Sleet {
    type Result = CellAck;

    fn handle(&mut self, msg: GetCell, _ctx: &mut Context<Self>) -> Self::Result {
        CellAck { cell: self.live_cells.get(&msg.cell_hash).map(|x| x.clone()) }
    }
}

/// A request structure to get all in-memory live-cell hashes, accepted by consensus (sleet-component).
/// `GetCell` request can be used individually, to get a specific cell from these cell hashes.
#[derive(Debug, Clone, Serialize, Deserialize, Message)]
#[rtype(result = "CellHashes")]
pub struct GetCellHashes;

#[derive(Debug, Clone, Serialize, Deserialize, MessageResponse)]
pub struct CellHashes {
    pub ids: Vec<CellHash>,
}

impl Handler<GetCellHashes> for Sleet {
    type Result = CellHashes;

    fn handle(&mut self, _msg: GetCellHashes, _ctx: &mut Context<Self>) -> Self::Result {
        return CellHashes { ids: self.live_cells.keys().cloned().collect::<Vec<CellHash>>() };
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Message)]
#[rtype(result = "AcceptedCellHashes")]
pub struct GetAcceptedCellHashes;

#[derive(Debug, Clone, Serialize, Deserialize, MessageResponse)]
pub struct AcceptedCellHashes {
    pub ids: Vec<CellHash>,
}

impl Handler<GetAcceptedCellHashes> for Sleet {
    type Result = AcceptedCellHashes;

    fn handle(&mut self, _msg: GetAcceptedCellHashes, _ctx: &mut Context<Self>) -> Self::Result {
        return AcceptedCellHashes {
            ids: self.accepted_txs.iter().cloned().collect::<Vec<CellHash>>(),
        };
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Message)]
#[rtype(result = "AcceptedCellAck")]
pub struct GetAcceptedCell {
    pub cell_hash: CellHash,
}

#[derive(Debug, Clone, Serialize, Deserialize, MessageResponse)]
pub struct AcceptedCellAck {
    pub cell: Option<Cell>,
}

impl Handler<GetAcceptedCell> for Sleet {
    type Result = AcceptedCellAck;

    fn handle(&mut self, msg: GetAcceptedCell, _ctx: &mut Context<Self>) -> Self::Result {
        if let Ok((_, tx)) = tx_storage::get_tx(&self.known_txs, msg.cell_hash) {
            AcceptedCellAck { cell: Some(tx.cell) }
        } else {
            AcceptedCellAck { cell: None }
        }
    }
}
