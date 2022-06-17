use super::router::{Router, RouterRequest};
use crate::channel::Channel;
use crate::protocol::network::{NetworkRequest, NetworkResponse};
use crate::tls::upgrader::Upgrader;
use crate::{Error, Result};
use tracing::{error, info};

use std::sync::Arc;

use actix::Addr;
use actix_rt::net::TcpStream;
use actix_service::fn_service;

use std::net::SocketAddr;

/// Implements a server for handling incoming connections.
pub struct Server {
    /// The ip address which this server binds to.
    ip: SocketAddr,
    /// The address of the router.
    router: Addr<Router>,
    upgrader: Arc<dyn Upgrader>,
}

impl Server {
    pub fn new(ip: SocketAddr, router: Addr<Router>, upgrader: Arc<dyn Upgrader>) -> Server {
        Server { ip, router, upgrader }
    }

    // Starts an actix server that listens for incoming connections.
    // Default thread count is the number of logical cpus
    pub async fn listen(&self) -> Result<()> {
        let ip = self.ip.clone();
        let router = self.router.clone();
        let upgrader = self.upgrader.clone();
        info!("listening on {:?}", ip);

        actix_server::Server::build()
            .bind("listener", ip, move || {
                let router = router.clone();
                let upgrader = upgrader.clone();

                // creates a service process that runs for each incoming connection
                fn_service(move |stream: TcpStream| {
                    let router = router.clone();
                    let upgrader = upgrader.clone();
                    async move { Server::process_stream(stream, router, upgrader).await }
                })
            })?
            .run()
            .await
            .map_err(|err| Error::IO(err))
    }

    // Processes the tcp stream and sends the request to the router
    async fn process_stream(
        stream: TcpStream,
        router: Addr<Router>,
        upgrader: Arc<dyn Upgrader>,
    ) -> Result<()> {
        let connection = upgrader.upgrade(stream).await?;
        // The ID generated from a TCP connection is next to useless,
        // however for TLS it safely identifies the peer
        let check_peer = upgrader.is_tls();
        let peer_id = connection.get_id().unwrap();
        let mut channel: Channel<NetworkResponse, NetworkRequest> =
            Channel::wrap(connection).unwrap();
        let (mut sender, mut receiver) = channel.split();
        let request = receiver.recv().await.unwrap();
        match request.clone() {
            Some(request) => {
                let response: Result<NetworkResponse> = router
                    .send(RouterRequest { peer_id, check_peer, request: request.clone() })
                    .await
                    .unwrap();
                match response {
                    Ok(response) =>
                    //debug!("sending response = {:?}", response);
                    {
                        sender.send(response).await.unwrap()
                    }
                    Err(err) => error!("{:?}", err),
                }
            }
            None => error!("received None"),
        }

        Ok(())
    }
}
