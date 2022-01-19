use super::router::Router;
use crate::channel::Channel;
use crate::protocol::{Request, Response};
use crate::{Result, Error};
use tracing::{debug, error, info};

use actix::Addr;
use actix_service::{fn_service, ServiceFactoryExt as _};
use actix_rt::net::TcpStream;

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

	pub async fn listen(&self) -> Result<()> {
		let ip = self.ip.clone();
		let router = self.router.clone();
		info!("listening on {:?}", ip);

		actix_server::Server::build()
		.bind("listener", ip, move || {
			let router = router.clone();

			fn_service(move |stream: TcpStream| {
				let router = router.clone();

                async move {
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
			})
			.map_err(|err| Error::IO(err))
		})?		
		.run()
		.await.map_err(|err| Error::IO(err))
	}
}