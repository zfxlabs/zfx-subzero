use crate::protocol::{Request, Response};
use crate::view::View;
use crate::ice::Ice;
use crate::chain::{alpha, alpha::Alpha};
use crate::sleet::Sleet;

use tracing::{info, debug, error};

use actix::{Actor, Context, Handler, Addr, ResponseFuture};

pub struct Router {
    view: Addr<View>,
    ice: Addr<Ice>,
    alpha: Addr<Alpha>,
    sleet: Addr<Sleet>,
}

impl Router {
    pub fn new(
	view: Addr<View>,
	ice: Addr<Ice>,
	alpha: Addr<Alpha>,
	sleet: Addr<Sleet>,
    ) -> Self {
	Router { view, ice, alpha, sleet }
    }
}

impl Actor for Router {
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut Context<Self>) {
	debug!("router> started");
    }
}

impl Handler<Request> for Router {
    type Result = ResponseFuture<Response>;

    fn handle(&mut self, msg: Request, ctx: &mut Context<Self>) -> Self::Result {
	let view = self.view.clone();
	let ice = self.ice.clone();
	let alpha = self.alpha.clone();
	let sleet = self.sleet.clone();
	Box::pin(async move {
	    match msg {
		// Handshake
		Request::Version(version) => {
		    debug!("routing Version -> View");
		    let version_ack = view.send(version).await.unwrap();
		    Response::VersionAck(version_ack)
		},
		// Ice external requests
		Request::Ping(ping) => {
		    debug!("routing Ping -> Ice");
		    let ack = ice.send(ping).await.unwrap();
		    Response::Ack(ack)
		},
		Request::GetLastAccepted => {
		    debug!("routing GetLastAccepted -> Alpha");
		    let last_accepted = alpha.send(alpha::GetLastAccepted).await.unwrap();
		    Response::LastAccepted(last_accepted)
		},
		// Sleet external requests
		Request::GetTx(get_tx) => {
		    debug!("routing GetTx -> Sleet");
		    let tx_ack = sleet.send(get_tx).await.unwrap();
		    Response::TxAck(tx_ack)
		},
		Request::ReceiveTx(receive_tx) => {
		    debug!("routing ReceiveTx -> Sleet");
		    let receive_tx_ack = sleet.send(receive_tx).await.unwrap();
		    Response::ReceiveTxAck(receive_tx_ack)
		},
		Request::QueryTx(query_tx) => {
		    debug!("routing QueryTx -> Sleet");
		    let query_tx_ack = sleet.send(query_tx).await.unwrap();
		    Response::QueryTxAck(query_tx_ack)
		},
		_ => {
		    error!("received unknown request / not implemented");
		    Response::Unknown
		},
	    }
	})
    }
}
