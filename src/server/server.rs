use super::router::Router;
use crate::channel::Channel;
use crate::protocol::{Request, Response};
use crate::{Error, Result};
use tracing::{error, info};

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
}

impl Server {
    pub fn new(ip: SocketAddr, router: Addr<Router>) -> Server {
        Server { ip, router }
    }

    // Starts an actix server that listens for incoming connections.
    // Default thread count is the number of logical cpus
    pub async fn listen(&self) -> Result<()> {
        let ip = self.ip.clone();
        let router = self.router.clone();
        info!("listening on {:?}", ip);

        actix_server::Server::build()
            .bind("listener", ip, move || {
                let router = router.clone();

                // creates a service process that runs for each incoming connection
                fn_service(move |stream: TcpStream| {
                    let router = router.clone();
                    async move { Server::process_stream(stream, router).await }
                })
            })?
            .run()
            .await
            .map_err(|err| Error::IO(err))
    }

    // Processes the tcp stream and sends the request to the router
    async fn process_stream(stream: TcpStream, router: Addr<Router>) -> Result<()> {
        let mut channel: Channel<Response, Request> = Channel::wrap(stream).unwrap();
        let (mut sender, mut receiver) = channel.split();
        let request = receiver.recv().await.unwrap();
        match request.clone() {
            Some(request) => {
                let response = router.send(request.clone()).await.unwrap();
                //debug!("sending response = {:?}", response);
                sender.send(response).await.unwrap();
            }
            None => error!("received None"),
        }

        Ok(())
    }
}
