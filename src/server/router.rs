use crate::p2p::prelude::*;

//use crate::hail::Hail;
use crate::ice::Ice;
use crate::message::{LastCellIdAck, Version, VersionAck, CURRENT_VERSION};
use crate::p2p::id::Id;
use crate::p2p::peer_meta::PeerMetadata;
use crate::protocol::graph::{GraphRequest, GraphResponse};
use crate::protocol::network::{NetworkRequest, NetworkResponse};
//use crate::sleet::Sleet;
use crate::{alpha, alpha::Alpha};

use tracing::{debug, error, info, trace, warn};

use std::collections::HashSet;
use std::sync::Arc;
use std::sync::RwLock;

//use crate::sleet;
//use actix::{Actor, Addr, AsyncContext, Context, Handler, ResponseFuture};

const MAX_PEER_SET: usize = 32;

#[derive(Clone)]
pub enum RouterState {
    /// The `Bootstrapping` state implies the set of peers in the `peer_set` are whitelisted.
    Bootstrapping,
    /// The `Ready` state implies the set of peers in the `peer_set` are validators.
    Ready,
}

/// The `Router` routes requests from peers to `network` protocol handlers.
pub struct Router {
    self_peer: PeerMetadata,
    peer_set: Arc<RwLock<HashSet<PeerMetadata>>>,
    alpha_address: Addr<Alpha>,
    ice_address: Option<Addr<Ice>>,
    state: RouterState,
}

impl Router {
    pub fn new(self_peer: PeerMetadata, alpha_address: Addr<Alpha>) -> Self {
        let mut initial_peer_set = HashSet::new();
        initial_peer_set.insert(self_peer.clone());
        Router {
            self_peer,
            peer_set: Arc::new(RwLock::new(initial_peer_set)),
            alpha_address,
            ice_address: None,
            state: RouterState::Bootstrapping,
        }
    }

    pub fn set_ice_address(&mut self, ice_address: Addr<Ice>) {
        self.ice_address = Some(ice_address);
    }
}

impl Actor for Router {
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut Context<Self>) {
        debug!("[router] started");
    }
}

#[derive(Clone, Message)]
#[rtype(result = "()")]
pub struct InitIce {
    pub addr: Addr<Ice>,
}

impl Handler<InitIce> for Router {
    type Result = ();

    fn handle(&mut self, InitIce { addr }: InitIce, _ctx: &mut Context<Self>) -> Self::Result {
        self.set_ice_address(addr);
    }
}

#[derive(Clone, Message)]
#[rtype(result = "()")]
pub struct TransitionReady;

impl Handler<TransitionReady> for Router {
    type Result = ();

    fn handle(&mut self, msg: TransitionReady, _ctx: &mut Context<Self>) -> Self::Result {
        self.state = RouterState::Ready;
    }
}

/// Wrapper for a `Request`, augmenting it with the peer's ID
#[derive(Debug, Clone, Serialize, Deserialize, Message)]
#[rtype(result = "Result<NetworkResponse>")]
pub struct RouterRequest {
    /// ID of the peer. meaningful only when using TLS where the ID is generated from the certificate
    /// presented during handshake
    pub peer_id: Id,
    /// Whether the peer ID needs to be checked
    pub check_peer: bool,
    /// The request received
    pub request: NetworkRequest,
}

impl Handler<RouterRequest> for Router {
    type Result = ResponseFuture<Result<NetworkResponse>>;

    fn handle(
        &mut self,
        RouterRequest { peer_id, check_peer, request }: RouterRequest,
        _ctx: &mut Context<Self>,
    ) -> Self::Result {
        let self_peer = self.self_peer.clone();
        let peer_set = self.peer_set.clone();
        let alpha_address = self.alpha_address.clone();
        let ice_address = self.ice_address.clone();
        let state = self.state.clone();
        Box::pin(async move {
            trace!("handling incoming msg: needs_checking: {}, id: {}", check_peer, peer_id,);
            match request {
                // Handshake
                NetworkRequest::Version(version) => match state {
                    RouterState::Bootstrapping => {
                        let mut peer_set = peer_set.write().unwrap();
                        if version.peer_set.len() <= MAX_PEER_SET {
                            for peer in version.peer_set.iter().cloned() {
                                if peer.clone() == self_peer.clone() {
                                    continue;
                                } else {
                                    if peer_set.len() <= MAX_PEER_SET {
                                        peer_set.insert(peer);
                                    }
                                }
                            }
                        }
                        Ok(NetworkResponse::VersionAck(VersionAck::new(
                            self_peer,
                            peer_set.clone(),
                        )))
                    }
                    RouterState::Ready => {
                        let peer_set = peer_set.read().unwrap();
                        Ok(NetworkResponse::VersionAck(VersionAck::new(
                            self_peer,
                            peer_set.clone(),
                        )))
                    }
                },

                // Ice external requests
                NetworkRequest::Ping(ping) => match state {
                    RouterState::Bootstrapping => {
                        warn!("ice: router_state == Bootstrapping");
                        Err(Error::Bootstrapping)
                    }
                    RouterState::Ready => match ice_address {
                        Some(ice_address) => {
                            debug!("routing Ping -> Ice");
                            let ping_ack = ice_address.send(ping).await.unwrap();
                            Ok(NetworkResponse::PingAck(ping_ack))
                        }
                        None => Err(Error::IceUninitialised),
                    },
                },

                // Alpha state bootstrapping
                NetworkRequest::GraphRequest(graph_request) => match graph_request {
                    GraphRequest::LastCellId(last_cell_id) => {
                        let ack = alpha_address.send(alpha::LastCellId).await.unwrap().unwrap();
                        Ok(NetworkResponse::GraphResponse(GraphResponse::LastCellIdAck(
                            LastCellIdAck::new(self_peer, ack),
                        )))
                    }
                    _ => Err(Error::UnknownRequest),
                },

                // Request::GetLastAccepted => {
                //     debug!("routing GetLastAccepted -> Alpha");
                //     let last_accepted = alpha.send(alpha::GetLastAccepted).await.unwrap();
                //     Response::LastAccepted(last_accepted)
                // }
                // Request::GetCellHashes => {
                //     debug!("routing GetCellHashes -> Alpha");
                //     let cell_hashes = sleet.send(sleet::GetCellHashes).await.unwrap();
                //     Response::CellHashes(cell_hashes)
                // }
                // // Sleet external requests
                // Request::GetCell(get_cell) => {
                //     debug!("routing GetCell -> Sleet");
                //     let cell_ack = sleet.send(get_cell).await.unwrap();
                //     Response::CellAck(cell_ack)
                // }
                // Request::GetAcceptedCellHashes => {
                //     debug!("routing GetAcceptedCellHashes -> Sleet");
                //     let cell_hashes = sleet
                //         .send(sleet::sleet_cell_handlers::GetAcceptedCellHashes)
                //         .await
                //         .unwrap();
                //     Response::AcceptedCellHashes(cell_hashes)
                // }
                // Request::GetAcceptedCell(get_cell) => {
                //     debug!("routing GetAcceptedCell -> Sleet");
                //     let cell_ack = sleet.send(get_cell).await.unwrap();
                //     Response::AcceptedCellAck(cell_ack)
                // }
                // Request::GenerateTx(generate_tx) => {
                //     debug!("routing GenerateTx -> Sleet");
                //     let receive_tx_ack = sleet.send(generate_tx).await.unwrap();
                //     Response::GenerateTxAck(receive_tx_ack)
                // }
                // Request::QueryTx(query_tx) => {
                //     // This request is only accepted from validators
                //     if check_peer && !validators.contains(&peer_id) {
                //         info!("Refusing validator request {:?} from peer {}", query_tx, peer_id);
                //         return Response::RequestRefused;
                //     }
                //     debug!("routing QueryTx -> Sleet");
                //     let query_tx_ack = sleet.send(query_tx).await.unwrap();
                //     Response::QueryTxAck(query_tx_ack)
                // }
                // Request::GetTxAncestors(get_ancestors) => {
                //     // This request is only accepted from validators
                //     if check_peer && !validators.contains(&peer_id) {
                //         info!(
                //             "Refusing validator request {:?} from peer {}",
                //             get_ancestors, peer_id
                //         );
                //         return Response::RequestRefused;
                //     }
                //     debug!("routing QueryTx -> Sleet");
                //     let ancestors = sleet.send(get_ancestors).await.unwrap();
                //     Response::TxAncestors(ancestors)
                // }
                // // Hail external requests
                // Request::GetBlock(get_block) => {
                //     debug!("routing GetBlock -> Hail");
                //     let block_ack = hail.send(get_block).await.unwrap();
                //     Response::BlockAck(block_ack)
                // }
                // Request::GetBlockByHeight(get_block) => {
                //     debug!("routing GetBlockByHeight -> Hail");
                //     let block_ack = hail.send(get_block).await.unwrap();
                //     Response::BlockAck(block_ack)
                // }
                // Request::QueryBlock(query_block) => {
                //     // This request is only accepted from validators
                //     if check_peer && !validators.contains(&peer_id) {
                //         info!("Refusing validator request {:?} from peer {}", query_block, peer_id);
                //         return Response::RequestRefused;
                //     }
                //     debug!("routing QueryBlock -> Hail");
                //     let query_block_ack = hail.send(query_block).await.unwrap();
                //     Response::QueryBlockAck(query_block_ack)
                // }
                // Request::CheckStatus => {
                //     debug!("routing CheckStatus -> Ice");
                //     let status = ice.send(CheckStatus).await.unwrap();
                //     Response::Status(status)
                // }
                req => Err(Error::UnknownRequest),
            }
        })
    }
}
