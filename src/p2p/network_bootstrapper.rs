use super::prelude::*;

use super::peer_bootstrapper::ReceivePeerGroup;

use std::collections::{HashMap, HashSet};

pub struct NetworkBootstrapper {
    /// The chain ids of the chains this network is subscribed to.
    chains: Vec<Id>,
    /// Map of chain id to bootstrap peer groups which can be used to bootstrap a chain.
    chain_bootstrap_peers: HashMap<Id, HashSet<PeerMetadata>>,
    /// Map of chain id to chain recipients.
    chain_bootstrap_recipients: HashMap<Id, Recipient<ReceiveBootstrapQuorum>>,
}

impl NetworkBootstrapper {
    pub fn new(chains: Vec<Id>) -> Self {
        NetworkBootstrapper {
            chains,
            chain_bootstrap_peers: HashMap::default(),
            chain_bootstrap_recipients: HashMap::default(),
        }
    }
}

impl Actor for NetworkBootstrapper {
    type Context = Context<Self>;

    fn stopped(&mut self, ctx: &mut Context<Self>) {
        info!("stopped");
    }
}

#[derive(Debug, Clone, Message)]
#[rtype(result = "()")]
pub struct ReceiveBootstrapQuorum {
    pub chain: Id,
    pub group: Vec<PeerMetadata>,
}

impl Handler<ReceivePeerGroup> for NetworkBootstrapper {
    type Result = ();

    fn handle(&mut self, msg: ReceivePeerGroup, ctx: &mut Context<Self>) -> Self::Result {
        //
    }
}
