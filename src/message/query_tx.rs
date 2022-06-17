use crate::p2p::peer_meta::PeerMetadata;
use crate::sleet::tx::{Tx, TxHash};

#[derive(Debug, Clone, Serialize, Deserialize, Message)]
#[rtype(result = "QueryTxAck")]
pub struct QueryTx {
    /// The querying nodes metadata.
    pub peer_meta: PeerMetadata,
    /// The contained `Tx`.
    pub tx: Tx,
}

impl QueryTx {
    pub fn new(peer_meta: PeerMetadata, tx: Tx) -> Self {
        QueryTx { peer_meta, tx }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, MessageResponse)]
pub struct QueryTxAck {
    /// The responding nodes metadata.
    pub peer_meta: PeerMetadata,
    /// The tx id being voted upon.
    pub tx_hash: TxHash,
    /// The outcome (true = vote for the tx, false = vote against the tx).
    pub outcome: bool,
}
