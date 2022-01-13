use crate::zfx_id::Id;

use actix::{Actor, Addr, Context, Handler};
use blake2::digest::{Update, VariableOutput}; // for hash function
use blake2::Blake2bVar;
use futures::TryFutureExt;
// for hash function
use priority_queue::double_priority_queue::DoublePriorityQueue;
use std::collections::HashMap;
use tracing::debug;

const GOSSIP_LIMIT: usize = 3; // Amount of gossip allowed to be passed

type GossipId = u64;

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

//FXIME: Perhaps generalised version?
pub struct PriorityMap {
    c: GossipId, // Counter can be pretty simple, since this only DC calls this struct
    h: HashMap<GossipId, Gossip>,
    q: DoublePriorityQueue<GossipId, u64>,
}

impl PriorityMap {
    pub fn new() -> PriorityMap {
        PriorityMap { c: 0, h: HashMap::new(), q: DoublePriorityQueue::new() }
    }

    pub fn push(&mut self, g: Gossip) {
        self.h.insert(self.c, g);
        self.q.push(self.c, 0);
        self.c += 1;
    }

    pub fn cleanup(&mut self, limit: u64) {
        while self.has_over_limit(&limit) {
            let (i, _p) = self.q.pop_max().unwrap();
            self.h.remove(&i);
        }
    }

    fn has_over_limit(&self, limit: &u64) -> bool {
        match self.q.peek_max() {
            None => false,
            Some((_i, p)) => p >= limit,
        }
    }

    pub fn take_n(&mut self, n: usize) -> Vec<Gossip> {
        let mut v: Vec<(GossipId, u64)> = vec![];

        for _ in 0..n {
            match self.q.pop_min() {
                None => break,
                Some(kv) => v.push(kv),
            }
        }

        for (id, p) in v.iter() {
            self.q.push_increase(*id, p + 1);
        }

        v.iter().map(|(id, _)| self.h.get(&id).unwrap().clone()).collect()
    }

    pub fn empty(&self) -> bool {
        self.q.len() == 0
    }

    pub fn len(&self) -> usize {
        self.q.len()
    }
}

pub struct DisseminationComponent {
    rumours: PriorityMap,
}

impl DisseminationComponent {
    fn new() -> Self {
        DisseminationComponent { rumours: PriorityMap::new() }
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
        self.rumours.push(msg);
        GossipAck {}
    }
}

impl Handler<GossipQuery> for DisseminationComponent {
    type Result = Rumours;

    fn handle(&mut self, msg: GossipQuery, _ctx: &mut Context<Self>) -> Self::Result {
        Rumours { rumours: self.rumours.take_n(GOSSIP_LIMIT) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::Rng;

    const N: usize = 10; // Number of gossip messages
    const NETWORK_SIZE: usize = 128; // Networks size in the test

    #[actix_rt::test]
    async fn test_pm_single() {
        let mut pm: PriorityMap = PriorityMap::new();
        assert!(pm.empty());
        assert_eq!(pm.len(), 0);

        let r = pm.take_n(1);
        assert_eq!(0, r.len());

        let slice = rand::thread_rng().gen::<[u8; 32]>(); // Random 32byte byte slice
        let id = Id::new(&slice);
        pm.push(Gossip::Leaver { id });
        assert!(!pm.empty());
        assert_eq!(pm.len(), 1);

        // Cleanup with large number -> won't delete
        pm.cleanup(10);
        assert!(!pm.empty());
        // Cleanup with 0 -> deletes all
        pm.cleanup(0);
        assert!(pm.empty());

        pm.push(Gossip::Leaver { id });
        // One item in the queue, one item in the response
        let r2 = pm.take_n(1);
        assert_eq!(r2.len(), 1);
        let r3 = pm.take_n(200);
        assert_eq!(r3.len(), 1);
        pm.cleanup(10);
        assert!(!pm.empty());
        // Cleanup with 0 -> deletes all
        pm.cleanup(2);
        assert!(pm.empty());
    }

    #[actix_rt::test]
    async fn test_pm_multi() {
        let mut pm: PriorityMap = PriorityMap::new();

        for _ in 0..100 {
            let slice = rand::thread_rng().gen::<[u8; 32]>(); // Random 32byte byte slice
            let id = Id::new(&slice);
            pm.push(Gossip::Leaver { id });
        }
        assert_eq!(pm.len(), 100);
        for _ in 0..10 {
            pm.take_n(10);
        }
        assert_eq!(pm.len(), 100);
        pm.cleanup(2);
        // Nothing reached prio = 2, so nothing has been deleted.
        // In other words, lowest prio is sent
        assert_eq!(pm.len(), 100);

        pm.take_n(50);
        assert_eq!(pm.len(), 100);
        // 50 items has been taken twice -> reached prio = 2 -> those will be deleted
        pm.cleanup(2);
        assert_eq!(pm.len(), 50);
    }

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

    #[actix_rt::test]
    async fn test_multi_push_pull() {
        let dc = DisseminationComponent::new();
        let dc_addr = dc.start();

        let mut ids: Vec<Id> = vec![];

        for i in 0..N {
            let slice = rand::thread_rng().gen::<[u8; 32]>(); // Random 32byte byte slice
            let id = Id::new(&slice);
            ids.push(id);
            match dc_addr.send(Gossip::Leaver { id: id.clone() }).await.unwrap() {
                GossipAck {} => (),
                _ => panic!("unexpected send result"),
            }
        }

        let pulls = ((N / GOSSIP_LIMIT) as usize + 1);

        for i in 0..pulls {
            let Rumours { mut rumours } = dc_addr.send(GossipQuery {}).await.unwrap();
            let len = rumours.len();
            if len > GOSSIP_LIMIT {
                panic!("unexpected rumours length {:?}", len);
            }
            if len == 0 {
                panic!("no rumours could be pulled");
            }
            for g in rumours {
                let Gossip::Leaver { id } = g;
                assert!(ids.contains(&id));
            }
        }
    }
}

// FIXME: Copied from zfx_id
// This function is the replacement for `zfx_crypto`s `hash!` macro
pub fn hash(input: &[u8]) -> [u8; 32] {
    let mut hasher = Blake2bVar::new(32).unwrap();
    hasher.update(input);
    let mut buf = [0u8; 32];
    hasher.finalize_variable(&mut buf).unwrap();
    buf
}
