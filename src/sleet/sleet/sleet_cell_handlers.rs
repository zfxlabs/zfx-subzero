use crate::cell::types::CellHash;
use crate::cell::Cell;
use crate::sleet::Sleet;
use crate::storage::tx as tx_storage;
use actix::{Context, Handler};

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
