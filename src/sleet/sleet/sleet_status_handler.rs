use crate::alpha::types::Weight;
use crate::sleet::Sleet;
use crate::zfx_id::Id;
use actix::{Context, Handler};
use std::net::SocketAddr;

#[derive(Debug, Clone, Serialize, Deserialize, Message)]
#[rtype(result = "Status")]
pub struct CheckStatus;

#[derive(Debug, Clone, Serialize, Deserialize, MessageResponse)]
pub struct Status {
    pub node_id: Id,
    pub validators: Vec<(Id, SocketAddr, Weight)>,
}

impl Handler<CheckStatus> for Sleet {
    type Result = Status;

    fn handle(&mut self, _msg: CheckStatus, _ctx: &mut Context<Self>) -> Self::Result {
        let validators = self
            .committee
            .iter()
            .map(|i| (i.0.clone(), i.1 .0, i.1 .1))
            .collect::<Vec<(Id, SocketAddr, Weight)>>();
        Status { node_id: self.node_id, validators }
    }
}
