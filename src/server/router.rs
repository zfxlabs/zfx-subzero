use crate::hail::Hail;
use crate::ice::Ice;
use crate::protocol::{Request, Response};
use crate::sleet::Sleet;
use crate::view::View;
use crate::zfx_id::Id;
use crate::{alpha, alpha::Alpha};

use tracing::{debug, error, info, trace};

use std::collections::HashSet;
use std::sync::Arc;

use crate::sleet;
use actix::{Actor, Addr, AsyncContext, Context, Handler, ResponseFuture};

pub struct Router {
    view: Addr<View>,
    ice: Addr<Ice>,
    alpha: Addr<Alpha>,
    sleet: Addr<Sleet>,
    hail: Addr<Hail>,
    validators: Arc<HashSet<Id>>,
}

impl Router {
    pub fn new(
        view: Addr<View>,
        ice: Addr<Ice>,
        alpha: Addr<Alpha>,
        sleet: Addr<Sleet>,
        hail: Addr<Hail>,
    ) -> Self {
        Router { view, ice, alpha, sleet, hail, validators: Arc::new(HashSet::new()) }
    }
}

impl Actor for Router {
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut Context<Self>) {
        self.alpha.do_send(InitRouter { addr: ctx.address() });
        debug!("router> started");
    }
}

#[derive(Clone, Message)]
#[rtype(result = "()")]
pub struct InitRouter {
    pub addr: Addr<Router>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Message)]
#[rtype(result = "()")]
pub struct ValidatorSet {
    pub validators: HashSet<Id>,
}

impl Handler<ValidatorSet> for Router {
    type Result = ();

    fn handle(
        &mut self,
        ValidatorSet { validators }: ValidatorSet,
        _ctx: &mut Context<Self>,
    ) -> Self::Result {
        self.validators = Arc::new(validators);
    }
}

/// Wrapper for a `Request`, augmenting it with the peer's ID
#[derive(Debug, Clone, Serialize, Deserialize, Message)]
#[rtype(result = "Response")]
pub struct RouterRequest {
    /// ID of the peer. meaningful only when using TLS where the ID is generated from the certificate
    /// presented during handshake
    pub peer_id: Id,
    /// Whether the peer ID needs to be checked
    pub check_peer: bool,
    /// The request received
    pub request: Request,
}

impl Handler<RouterRequest> for Router {
    type Result = ResponseFuture<Response>;

    fn handle(
        &mut self,
        RouterRequest { peer_id, check_peer, request }: RouterRequest,
        _ctx: &mut Context<Self>,
    ) -> Self::Result {
        let view = self.view.clone();
        let ice = self.ice.clone();
        let alpha = self.alpha.clone();
        let sleet = self.sleet.clone();
        let hail = self.hail.clone();
        let validators = self.validators.clone();
        Box::pin(async move {
            trace!(
                "Handling incoming msg: needs_checking: {}, id: {}, validator: {}",
                check_peer,
                peer_id,
                validators.contains(&peer_id)
            );
            match request {
                // Handshake
                Request::Version(version) => {
                    debug!("routing Version -> View");
                    let version_ack = view.send(version).await.unwrap();
                    Response::VersionAck(version_ack)
                }
                // Ice external requests
                Request::Ping(ping) => {
                    debug!("routing Ping -> Ice");
                    let ack = ice.send(ping).await.unwrap();
                    Response::Ack(ack)
                }
                Request::GetLastAccepted => {
                    debug!("routing GetLastAccepted -> Alpha");
                    let last_accepted = alpha.send(alpha::GetLastAccepted).await.unwrap();
                    Response::LastAccepted(last_accepted)
                }
                Request::GetCellHashes => {
                    debug!("routing GetCellHashes -> Alpha");
                    let cell_hashes = sleet.send(sleet::GetCellHashes).await.unwrap();
                    Response::CellHashes(cell_hashes)
                }
                // Sleet external requests
                Request::GetCell(get_cell) => {
                    debug!("routing GetCell -> Sleet");
                    let cell_ack = sleet.send(get_cell).await.unwrap();
                    Response::CellAck(cell_ack)
                }
                Request::GetAcceptedCellHashes => {
                    debug!("routing GetAcceptedCellHashes -> Sleet");
                    let cell_hashes = sleet
                        .send(sleet::sleet_cell_handlers::GetAcceptedCellHashes)
                        .await
                        .unwrap();
                    Response::AcceptedCellHashes(cell_hashes)
                }
                Request::GetAcceptedCell(get_cell) => {
                    debug!("routing GetAcceptedCell -> Sleet");
                    let cell_ack = sleet.send(get_cell).await.unwrap();
                    Response::AcceptedCellAck(cell_ack)
                }
                Request::GenerateTx(generate_tx) => {
                    debug!("routing GenerateTx -> Sleet");
                    let receive_tx_ack = sleet.send(generate_tx).await.unwrap();
                    Response::GenerateTxAck(receive_tx_ack)
                }
                Request::QueryTx(query_tx) => {
                    // This request is only accepted from validators
                    if check_peer && !validators.contains(&peer_id) {
                        info!("Refusing validator request {:?} from peer {}", query_tx, peer_id);
                        return Response::RequestRefused;
                    }
                    debug!("routing QueryTx -> Sleet");
                    let query_tx_ack = sleet.send(query_tx).await.unwrap();
                    Response::QueryTxAck(query_tx_ack)
                }
                Request::GetTxAncestors(get_ancestors) => {
                    // This request is only accepted from validators
                    if check_peer && !validators.contains(&peer_id) {
                        info!(
                            "Refusing validator request {:?} from peer {}",
                            get_ancestors, peer_id
                        );
                        return Response::RequestRefused;
                    }
                    debug!("routing QueryTx -> Sleet");
                    let ancestors = sleet.send(get_ancestors).await.unwrap();
                    Response::TxAncestors(ancestors)
                }
                // Hail external requests
                Request::GetBlock(get_block) => {
                    debug!("routing GetBlock -> Hail");
                    let block_ack = hail.send(get_block).await.unwrap();
                    Response::BlockAck(block_ack)
                }
                Request::GetBlockByHeight(get_block) => {
                    debug!("routing GetBlockByHeight -> Hail");
                    let block_ack = hail.send(get_block).await.unwrap();
                    Response::BlockAck(block_ack)
                }
                Request::QueryBlock(query_block) => {
                    // This request is only accepted from validators
                    if check_peer && !validators.contains(&peer_id) {
                        info!("Refusing validator request {:?} from peer {}", query_block, peer_id);
                        return Response::RequestRefused;
                    }
                    debug!("routing QueryBlock -> Hail");
                    let query_block_ack = hail.send(query_block).await.unwrap();
                    Response::QueryBlockAck(query_block_ack)
                }
                Request::GetNodeStatus => {
                    debug!("routing GetNodeStatus -> Alpha");
                    let status =
                        alpha.send(alpha::status_handler::GetNodeStatus).await.unwrap().unwrap();
                    Response::NodeStatus(status)
                }
                req => {
                    error!("received unknown request / not implemented = {:?}", req);
                    Response::Unknown
                }
            }
        })
    }
}
