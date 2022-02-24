use crate::hail::Hail;
use crate::ice::{CheckStatus, Ice};
use crate::protocol::{Request, Response};
use crate::sleet::Sleet;
use crate::view::View;
use crate::{alpha, alpha::Alpha};

use tracing::{debug, error, info};

use crate::sleet;
use actix::{Actor, Addr, Context, Handler, ResponseFuture};

pub struct Router {
    view: Addr<View>,
    ice: Addr<Ice>,
    alpha: Addr<Alpha>,
    sleet: Addr<Sleet>,
    hail: Addr<Hail>,
}

impl Router {
    pub fn new(
        view: Addr<View>,
        ice: Addr<Ice>,
        alpha: Addr<Alpha>,
        sleet: Addr<Sleet>,
        hail: Addr<Hail>,
    ) -> Self {
        Router { view, ice, alpha, sleet, hail }
    }
}

impl Actor for Router {
    type Context = Context<Self>;

    fn started(&mut self, _ctx: &mut Context<Self>) {
        debug!("router> started");
    }
}

impl Handler<Request> for Router {
    type Result = ResponseFuture<Response>;

    fn handle(&mut self, msg: Request, _ctx: &mut Context<Self>) -> Self::Result {
        let view = self.view.clone();
        let ice = self.ice.clone();
        let alpha = self.alpha.clone();
        let sleet = self.sleet.clone();
        let hail = self.hail.clone();
        Box::pin(async move {
            match msg {
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
                    debug!("routing QueryTx -> Sleet");
                    let query_tx_ack = sleet.send(query_tx).await.unwrap();
                    Response::QueryTxAck(query_tx_ack)
                }
                Request::GetTxAncestors(get_ancestors) => {
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
                    debug!("routing QueryBlock -> Hail");
                    let query_block_ack = hail.send(query_block).await.unwrap();
                    Response::QueryBlockAck(query_block_ack)
                }
                Request::CheckStatus => {
                    debug!("routing CheckStatus -> Ice");
                    let status = ice.send(CheckStatus).await.unwrap();
                    Response::Status(status)
                }
                req => {
                    error!("received unknown request / not implemented = {:?}", req);
                    Response::Unknown
                }
            }
        })
    }
}
