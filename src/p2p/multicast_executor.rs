use super::linear_backoff::Execute;
use super::multicast::{Multicast, MulticastRequest};
use crate::p2p::prelude::*;
use crate::protocol::{Request, Response};

pub struct MulticastExecutor<Req: Request, Rsp: Response> {
    request: Req,
    multicast: Multicast<Rsp>,
    iteration: usize,
    iteration_limit: usize,
    finished: bool,
}

impl<Req: Request, Rsp: Response> MulticastExecutor<Req, Rsp> {
    pub fn new(request: Req, multicast: Multicast<Rsp>, iteration_limit: usize) -> Self {
        MulticastExecutor { request, multicast, iteration: 0, iteration_limit, finished: false }
    }
}

impl<Req: Request, Rsp: Response> Actor for MulticastExecutor<Req, Rsp> {
    type Context = Context<Self>;
}

impl<Req: Request, Rsp: Response> Handler<Execute> for MulticastExecutor<Req, Rsp> {
    type Result = ResponseFuture<bool>;

    fn handle(&mut self, msg: Execute, ctx: &mut Context<Self>) -> Self::Result {
        let self_recipient = ctx.address().recipient().clone();
        if !self.finished {
            if self.iteration > self.iteration_limit {
                warn!("multicast repeated beyond the iteration limit");
                Box::pin(async { true })
            } else if self.iteration == self.iteration_limit {
                info!("reached iteration limit");
                self.iteration += 1;
                self.finished = true;
                Box::pin(async { true })
            } else {
                self.iteration += 1;
                Box::pin(async move {
                    // info!("multicasting {:?}", self.request.clone());
                    // let _ = self.multicast.send(MulticastRequest { request: self.request.clone() }).await.unwrap();
                    false
                })
            }
        } else {
            Box::pin(async { false })
        }
    }
}
