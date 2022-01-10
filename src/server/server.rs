use crate::zfx_id::Id;

use super::router::Router;
use crate::channel::Channel;
use crate::protocol::{Request, Response};
use crate::Result;
use tracing::{error, info};

use actix::Addr;

use std::net::SocketAddr;
use tokio::net::TcpListener;

fn id_from_ip(ip: &SocketAddr) -> Id {
    Id::new(format!("{:?}", ip.clone()).as_bytes())
}

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

    pub async fn listen(self) -> Result<()> {
        let listener = TcpListener::bind(self.ip.clone()).await?;
        info!("listening on {:?}", self.ip.clone());
        loop {
            let ip = self.ip.clone();
            let self_id = id_from_ip(&ip);
            let router = self.router.clone();
            let mut channel: Channel<Response, Request> = Channel::accept(&listener).await?;
            tokio::spawn(async move {
                let (mut sender, mut receiver) = channel.split();
                // receive a request
                let request = receiver.recv().await.unwrap();
                // process the request
                match request.clone() {
                    Some(request) => {
                        let response = router.send(request).await.unwrap();
                        // debug!("sending response = {:?}", response);
                        sender.send(response).await.unwrap();
                    }
                    None => error!("received None"),
                }
            });
        }
    }
}
