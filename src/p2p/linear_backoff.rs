use super::prelude::*;

/// The `LinearBackoff` Actor sends `Execute` messages at linearly increasing time
/// intervals whilst counting each execution and storing it in its `count` field.
///
/// The purpose of a backoff function is to find an epoch duration (subsequently referred to
/// as `delta`) which a network of nodes share such that messages can be delivered and
/// liveness can be maintained (under standard byzantine fault tolerant assumptions).
pub struct LinearBackoff {
    executor: Recipient<Execute>,
    epoch: u32,
    delta: Duration,
}

impl LinearBackoff {
    /// When creating a `LinearBackoff` one may specify the `Actor` which receives
    /// `Execute` messages within an `epoch` of duration `delta`.
    pub fn new(executor: Recipient<Execute>, delta: Duration) -> Self {
        LinearBackoff { executor, epoch: 0, delta }
    }
}

/// The `LinearBackoff` `Actor` may be sent a `Start` message in order to begin
/// periodically sending `Execute` messages to another `Actor`.
#[derive(Debug, Clone, Message)]
#[rtype(result = "()")]
pub struct Start;

impl Handler<Start> for LinearBackoff {
    type Result = ResponseActFuture<Self, ()>;

    fn handle(&mut self, msg: Start, ctx: &mut Context<Self>) -> Self::Result {
        let send_to_executor = self.executor.send(Execute);
        let send_to_executor = actix::fut::wrap_future::<_, Self>(send_to_executor);
        Box::pin(send_to_executor.map(move |done, actor, ctx| match done {
            Ok(done) => {
                if !done {
                    actor.epoch += 1;
                    let delta = actor.delta.clone() * actor.epoch;
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

/// The `Actor` sends `Execute` messages to actors at the start of some epoch. The actor
/// handling the `Execute` message should return `true` when the backoff should complete
/// and `false` when it should be repeated.
#[derive(Debug, Clone, Message)]
#[rtype(result = "bool")]
pub struct Execute;

impl Actor for LinearBackoff {
    type Context = Context<Self>;

    fn stopped(&mut self, ctx: &mut Context<Self>) {
        debug!("stopped");
    }
}
