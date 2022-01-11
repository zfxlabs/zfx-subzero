use crate::zfx_id::Id;

use actix::{Actor, Addr, Context, Handler};
use tracing::debug;

const GOSSIP_LIMIT: usize = 8; // Amount of gossip allowed to be passed

#[derive(Debug, Clone, Message)]
#[rtype(result = "GossipAck")]
pub enum Gossip {
    Leaver { id: Id },
}

#[derive(Debug, Clone, MessageResponse)]
pub struct GossipAck {}

#[derive(Debug, Clone, Message)]
#[rtype(result = "Rumours")]
pub struct GossipQuery {}

#[derive(Debug, Clone, MessageResponse)]
pub struct Rumours {
    rumours: Vec<Gossip>,
}

pub struct DisseminationComponent {
    gossip_queue: Vec<Gossip>,
}

impl DisseminationComponent {
    fn new() -> Self {
        DisseminationComponent { gossip_queue: vec![] }
    }
}

impl Actor for DisseminationComponent {
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut Context<Self>) {
        debug!(":started");
    }
}

impl Handler<Gossip> for DisseminationComponent {
    type Result = GossipAck;

    fn handle(&mut self, msg: Gossip, _ctx: &mut Context<Self>) -> Self::Result {
        self.gossip_queue.push(msg);
        GossipAck {}
    }
}

impl Handler<GossipQuery> for DisseminationComponent {
    type Result = Rumours;

    fn handle(&mut self, msg: GossipQuery, _ctx: &mut Context<Self>) -> Self::Result {
        let rumours_to_pass: usize = std::cmp::min(self.gossip_queue.len(), GOSSIP_LIMIT);
        Rumours { rumours: self.gossip_queue[..rumours_to_pass].to_vec() }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[actix_rt::test]
    async fn test_simple_push_pull() {
        let dc = DisseminationComponent::new();
        let dc_addr = dc.start();

        let stored_id = Id::new(&[0; 32]);

        match dc_addr.send(Gossip::Leaver { id: stored_id.clone() }).await.unwrap() {
            GossipAck {} => (),
            _ => panic!("unexpected send result"),
        }

        let Rumours { mut rumours } = dc_addr.send(GossipQuery {}).await.unwrap();
        assert_eq!(rumours.len(), 1);
        let Gossip::Leaver { id } = rumours.pop().unwrap();
        assert_eq!(id, stored_id);
    }
}
