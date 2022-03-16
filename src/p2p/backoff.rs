use actix::{AsyncContext, Context};
use actix::{Actor, Handler, ResponseFuture, Recipient};
use actix::{ActorFutureExt, ResponseActFuture, WrapFuture};

use tracing::{info, warn, error};

use tokio::time::Duration;

// The soft backoff actor sends `Execute` messages to actors at the start of some epoch the executor
// returns `false`.

pub struct Backoff {
    executor: Recipient<Execute>,
    count: usize,
    delta: Duration,
}

impl Backoff {
    pub fn new(executor: Recipient<Execute>, delta: Duration) -> Self {
	Backoff { executor, count: 0, delta }
    }
}

#[derive(Debug, Clone, Message)]
#[rtype(result = "()")]
pub struct Start;

#[derive(Debug, Clone, Message)]
#[rtype(result = "bool")]
pub struct Execute;

impl Actor for Backoff {
    type Context = Context<Self>;

    fn stopped(&mut self, ctx: &mut Context<Self>) {
	info!("[backoff] stopped");
    }
}

impl Handler<Start> for Backoff {
    type Result = ResponseActFuture<Self, ()>;

    fn handle(&mut self, msg: Start, ctx: &mut Context<Self>) -> Self::Result {
	let send_to_executor = self.executor.send(Execute);
	let send_to_executor = actix::fut::wrap_future::<_, Self>(send_to_executor);
	Box::pin(send_to_executor.map(move |done, actor, ctx| {
	    match done {
		Ok(done) => {
		    actor.count += 1;
		    ctx.notify_later(msg.clone(), actor.delta.clone());
		},
		Err(err) => {
		    error!("{:?}", err);
		    ()
		}
	    }
	}))
    }
}
