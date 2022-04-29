use super::prelude::*;

// The purpose of a backoff function is to expand the time bound between protocol epochs such that
// a point of synchrony is found between remote peers. This helps to find a suitable GST across
// nodes to achieve `2f + 1`.
//
// The backoff actor sends `Execute` messages to actors at the start of some epoch. The actor
// handling the `Execute` message should return `true` when the backoff should complete and
// `false` when it should be repeated.

pub struct LinearBackoff {
    executor: Recipient<Execute>,
    count: u32,
    delta: Duration,
}

impl LinearBackoff {
    pub fn new(executor: Recipient<Execute>, delta: Duration) -> Self {
        LinearBackoff { executor, count: 0, delta }
    }
}

#[derive(Debug, Clone, Message)]
#[rtype(result = "()")]
pub struct Start;

#[derive(Debug, Clone, Message)]
#[rtype(result = "bool")]
pub struct Execute;

impl Actor for LinearBackoff {
    type Context = Context<Self>;

    fn stopped(&mut self, ctx: &mut Context<Self>) {
        debug!("stopped");
    }
}

impl Handler<Start> for LinearBackoff {
    type Result = ResponseActFuture<Self, ()>;

    fn handle(&mut self, msg: Start, ctx: &mut Context<Self>) -> Self::Result {
        let send_to_executor = self.executor.send(Execute);
        let send_to_executor = actix::fut::wrap_future::<_, Self>(send_to_executor);
        Box::pin(send_to_executor.map(move |done, actor, ctx| match done {
            Ok(done) => {
                if !done {
                    actor.count += 1;
                    let delta = actor.delta.clone() * actor.count;
                    ctx.notify_later(msg.clone(), delta);
                }
            }
            Err(err) => {
                error!("{:?}", err);
                ()
            }
        }))
    }
}
