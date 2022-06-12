use super::prelude::*;

use super::connection::{Connection, Upgraded};
use super::response_handler::ResponseHandler;

use crate::channel::Channel;

#[derive(Clone)]
pub struct ConnectionHandler {
    request: Request,
    send_timeout: Duration,
    response_handler: Arc<dyn ResponseHandler>,
}

impl ConnectionHandler {
    pub fn new(
        request: Request,
        send_timeout: Duration,
        response_handler: Arc<dyn ResponseHandler>,
    ) -> Self {
        ConnectionHandler { request, send_timeout, response_handler }
    }

    pub fn handle_connection(
        &self,
        connection: Connection<Upgraded>,
    ) -> Pin<Box<dyn Future<Output = Result<()>>>> {
        let request = self.request.clone();
        let response_handler = self.response_handler.clone();
        let send_timeout = self.send_timeout.clone();
        let mut channel: Channel<Request, Response> =
            Channel::wrap(connection.state.connection_stream).unwrap();
        let (mut sender, mut receiver) = channel.split();
        Box::pin(async move {
            let () = sender.send(request.clone()).await.unwrap();
            //info!("-> {:?} ({})", request, "ok".green());
            match timeout(send_timeout, receiver.recv()).await {
                Ok(res) => match res {
                    Ok(Some(response)) => {
                        //info!("<- {:?} ({})", response.clone(), "ok".green());
                        response_handler.handle_response(response).await
                    }
                    Ok(None) => {
                        error!("{}", "empty_response".red());
                        Err(Error::EmptyResponse)
                    }
                    Err(err) => {
                        error!("{:?}", err);
                        Err(err.into())
                    }
                },
                Err(_) => {
                    warn!("timeout ({})", "warning".yellow());
                    Err(Error::Timeout)
                }
            }
        })
    }
}
