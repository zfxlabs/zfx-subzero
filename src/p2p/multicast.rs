//! Defines a `Multicast` actor used to multicast a message to peers and aggregate responses.
//! A multicast sends the same message to `k` peers.
use super::prelude::*;
use super::response_handler::ResponseHandler;
use super::sender::{multicast, Sender};
use crate::protocol::{Request, Response};

use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

pub struct Multicast<Rsp: Response> {
    /// Connection upgrader.
    upgrader: Arc<dyn Upgrader>,
    /// Peers to multicast to.
    peer_set: HashSet<PeerMetadata>,
    /// Responses received from peers (or requests which timed out).
    received: Vec<Rsp>,
    /// The recipient of the multicast responses once they have all been resolved.
    multicast_recipient: Recipient<MulticastResult<Rsp>>,
    /// Extent of time which a single send is allowed to take.
    timeout: Duration,
}

impl<Rsp: Response> Multicast<Rsp> {
    pub fn new(
        upgrader: Arc<dyn Upgrader>,
        peer_set: HashSet<PeerMetadata>,
        multicast_recipient: Recipient<MulticastResult<Rsp>>,
        timeout: Duration,
    ) -> Self {
        Multicast { upgrader, peer_set, received: vec![], multicast_recipient, timeout }
    }
}

#[derive(Debug, Clone, Message)]
#[rtype(result = "Result<()>")]
pub struct MulticastRequest<Req: Request> {
    pub request: Req,
}

#[derive(Debug, Clone, Message)]
#[rtype(result = "Result<()>")]
pub struct Part<Rsp: Response> {
    part: Rsp,
}

#[derive(Debug, Clone, Message)]
#[rtype(result = "()")]
pub struct MulticastResult<Rsp: Response> {
    pub result: Vec<Rsp>,
}

impl<Rsp: Response> Actor for Multicast<Rsp> {
    type Context = Context<Self>;
}

impl<Req: Request, Rsp: Response> Handler<MulticastRequest<Req>> for Multicast<Rsp> {
    type Result = ResponseActFuture<Self, Result<()>>;

    fn handle(&mut self, msg: MulticastRequest<Req>, ctx: &mut Context<Self>) -> Self::Result {
        let self_recipient = ctx.address().recipient().clone();
        let ack_handler = AckHandler::new(self_recipient);
        let sender_address = Sender::new(self.upgrader.clone(), ack_handler.clone()).start();
        let multicast_fut = multicast::<Req, Rsp>(
            sender_address,
            self.peer_set.clone(),
            msg.request,
            self.timeout.clone(),
        );
        let multicast_wrapped = actix::fut::wrap_future::<_, Self>(multicast_fut);
        Box::pin(multicast_wrapped)
    }
}

impl<Rsp: Response> Handler<Part<Rsp>> for Multicast<Rsp> {
    type Result = ResponseFuture<Result<()>>;

    fn handle(&mut self, msg: Part<Rsp>, ctx: &mut Context<Self>) -> Self::Result {
        // Add the received part to the `received` vector
        self.received.push(msg.part);
        // If the received length is greater than the peer set then this is an overflow error
        if self.received.len() > self.peer_set.len() {
            return Box::pin(async { Err(Error::MulticastOverflow) });
        }
        // If the received length is the same as the peer set then we are done
        if self.received.len() == self.peer_set.len() {
            info!("sending {:?} to multicast recipient", self.received.clone());
            let multicast_recipient = self.multicast_recipient.clone();
            let received = self.received.clone();
            let send_to_multicast_recipient = async move {
                multicast_recipient
                    .send(MulticastResult { result: received })
                    .await
                    .map_err(|err| err.into())
            };
            return Box::pin(send_to_multicast_recipient);
        }
        // Otherwise wait for more parts to be received (ok)
        Box::pin(async { Ok(()) })
    }
}

pub struct AckHandler<Rsp: Response> {
    recipient: Recipient<Part<Rsp>>,
}

impl<Rsp: Response> AckHandler<Rsp> {
    pub fn new(recipient: Recipient<Part<Rsp>>) -> Arc<dyn ResponseHandler<Rsp>> {
        Arc::new(AckHandler { recipient })
    }
}

impl<Rsp: Response> ResponseHandler<Rsp> for AckHandler<Rsp> {
    fn handle_response(&self, response: Rsp) -> Pin<Box<dyn Future<Output = Result<()>>>> {
        let recipient = self.recipient.clone();
        Box::pin(async move { recipient.send(Part { part: response }).await.unwrap() })
    }
}
