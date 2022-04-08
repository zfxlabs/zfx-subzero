use super::Result;
use crate::alpha::types::Weight;
use crate::alpha::Alpha;
use crate::ice::Choice;
use crate::zfx_id::Id;
use crate::{ice, sleet};
use actix::{ActorFutureExt, Context, Handler, ResponseActFuture, WrapFuture};
use std::net::SocketAddr;

#[derive(Debug, Clone, Serialize, Deserialize, Message)]
#[rtype(result = "Result<NodeStatus>")]
pub struct GetNodeStatus;

#[derive(Debug, Clone, Serialize, Deserialize, MessageResponse)]
pub struct NodeStatus {
    pub bootstrapped: bool,
    pub height: u64,
    pub peers: Vec<(Id, SocketAddr, Choice)>,
    pub validators: Vec<(Id, SocketAddr, Weight)>,
}

impl Handler<GetNodeStatus> for Alpha {
    type Result = ResponseActFuture<Self, Result<NodeStatus>>;

    fn handle(&mut self, _msg: GetNodeStatus, _ctx: &mut Context<Self>) -> Self::Result {
        let height = self.state.height;
        let ice_clone = self.ice.clone();
        let sleet_clone = self.sleet.clone();
        Box::pin(
            async move {
                let ice_status: ice::Status = ice_clone.send(ice::CheckStatus).await.unwrap();
                let sleet_status: sleet::sleet_status_handler::Status =
                    sleet_clone.send(sleet::sleet_status_handler::CheckStatus).await.unwrap();

                Ok(NodeStatus {
                    height,
                    bootstrapped: ice_status.bootstrapped,
                    peers: ice_status.peers,
                    validators: sleet_status.validators,
                })
            }
            .into_actor(self)
            .map(move |result, _actor, _ctx| result),
        )
    }
}
