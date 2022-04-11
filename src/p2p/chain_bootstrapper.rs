use super::prelude::*;

use std::collections::HashSet;

// trait ChainStorage {
//     fn exists(&self) -> bool;

//     fn genesis(&self) -> Box<dyn ChainElement>;

//     fn accept(&mut self, chain_element: Box<dyn ChainElement>) -> ();

//     fn safe_accept(&mut self, chain_element: Box<dyn ChainElement>) -> ();

//     fn last_accepted(&self) -> (Hash, Box<dyn ChainElement>);

//     fn is_known(&self, hash: Hash) -> bool;

//     fn range(&self) -> Vec<Box<dyn ChainElement>>;
// }

/// The chain bootstrapper is used to bootstrap the chain state from a set of trusted peers.
pub struct ChainBootstrapper {
    chain_id: Id,
    peers: HashSet<PeerMetadata>,
    recipient: Recipient<ReceiveBootstrapped>,
}

impl ChainBootstrapper {
    pub fn new(
        chain_id: Id,
        peers: HashSet<PeerMetadata>,
        recipient: Recipient<ReceiveBootstrapped>,
    ) -> Self {
        ChainBootstrapper { chain_id, peers, recipient }
    }
}

impl Actor for ChainBootstrapper {
    type Context = Context<Self>;

    fn stopped(&mut self, ctx: &mut Context<Self>) {
        info!("stopped");
    }
}

#[derive(Debug, Clone, Message)]
#[rtype(result = "()")]
pub struct ReceiveBootstrapped {
    pub chain: Id,
    pub group: Vec<PeerMetadata>,
}

// impl Handler<ReceiveBootstrapPeers> for ChainBootstrapper {
//      type Result = ();

//     fn handle(&mut self, msg: ReceiveBootstrapPeers, ctx: &mut Context<Self>) -> Self::Result {

//     }
// }
