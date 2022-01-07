use crate::zfx_id::Id;
use crate::util;
use crate::client;
use crate::version::{Version, VersionAck};
use crate::protocol::{Request, Response};
use crate::ice::{self, Ice};
use super::sampleable_map::SampleableMap;
use tracing::{debug, info};

use actix::{Actor, Handler, Addr, Context, ResponseFuture};

use std::net::SocketAddr;
use std::collections::HashSet;

const PEER_LIST_MAX: usize = 3;
const BOOTSTRAP_QUORUM: usize = 2;

/// The view contains the most up to date set of peer metadata.
#[derive(Debug)]
pub struct View {
    ip: SocketAddr,
    peers: SampleableMap<Id, SocketAddr>,
    peer_list: HashSet<(Id, SocketAddr)>,
}

impl std::ops::Deref for View {
    type Target = SampleableMap<Id, SocketAddr>;

    fn deref(&self) -> &'_ Self::Target {
        &self.peers
    }
}

impl std::ops::DerefMut for View {
    fn deref_mut(&mut self) -> &'_ mut Self::Target {
        &mut self.peers
    }
}

impl View {
    pub fn new(ip: SocketAddr) -> Self {
        Self { ip, peers: SampleableMap::new(), peer_list: HashSet::new() }
    }

    pub fn init(&mut self, ips: Vec<SocketAddr>) {
	for ip in ips.iter() {
	    let id = util::id_from_ip(ip);
    	    if let None = self.insert(id, ip.clone()) {
		debug!("inserted <id: {:?}, ip: {:?}>", id, ip.clone());
	    }
	    if self.peer_list.len() < PEER_LIST_MAX {
		if !self.peer_list.contains(&(id, ip.clone())) {
		    debug!("inserting <id: {:?}, ip: {:?}> in peer list", id, ip.clone());
		    self.peer_list.insert((id, ip.clone()));
		}
	    }
	}
    }

    // Returns whether the element was updated or not (if the element was missing)
    pub fn insert_update(&mut self, id: Id, ip: SocketAddr) -> bool {
	if id == util::id_from_ip(&self.ip) {
	    return false;
	}
	match self.insert(id, ip.clone()) {
	    Some(_) => {
		false
	    },
	    None => {
		debug!("inserted <id: {:?}, ip: {:?}>", id, ip.clone());
		if self.peer_list.len() < PEER_LIST_MAX {
		    if !self.peer_list.contains(&(id, ip.clone())) {
			self.peer_list.insert((id, ip.clone()));
			debug!("inserted <id: {:?}, ip: {:?}> in peer list", id, ip.clone());
		    }
		}
		true
	    }
	}
    }

    pub fn sample_k(&mut self, k: usize) -> Vec<(Id, SocketAddr)> {
        if self.len() > k {
            self.sample(k)
        } else {
            vec![]
        }
    }
}

impl Actor for View {
    type Context = Context<Self>;

    fn started(&mut self, _ctx: &mut Context<Self>) {
	debug!(": started")
    }
}

impl Handler<Version> for View {
    type Result = VersionAck;

    fn handle(&mut self, msg: Version, _ctx: &mut Context<Self>) -> Self::Result {
	// TODO: verify / extend `Version`
	let ip = msg.ip.clone();
	let id = util::id_from_ip(&ip);
	let _ = self.insert_update(id, ip);

	// Fetch the peer list
	let mut peer_vec = vec![];
	for peer in self.peer_list.iter().cloned() {
	    peer_vec.push(peer);
	}
	VersionAck { ip: self.ip.clone(), peer_list: peer_vec }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Message)]
#[rtype(result = "PeersResult")]
pub struct GetPeers;

#[derive(Debug, Clone, Serialize, Deserialize, MessageResponse)]
pub struct PeersResult {
    pub peers: Vec<(Id, SocketAddr)>,
}

impl Handler<GetPeers> for View {
    type Result = PeersResult;

    fn handle(&mut self, _msg: GetPeers, _ctx: &mut Context<Self>) -> Self::Result {
	let mut peer_vec = vec![];
	for (id, ip) in self.iter() {
	    peer_vec.push((id.clone(), ip.clone()));
	}
	PeersResult { peers: peer_vec }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Message)]
#[rtype(result = "BootstrapResult")]
pub struct Bootstrap;

#[derive(Debug, Clone, Serialize, Deserialize, MessageResponse)]
pub struct BootstrapResult {
    responses: Vec<Response>
}

impl Handler<Bootstrap> for View {
    type Result = ResponseFuture<BootstrapResult>;

    fn handle(&mut self, _msg: Bootstrap, _ctx: &mut Context<Self>) -> Self::Result {
	let ip = self.ip.clone();
	let id = util::id_from_ip(&ip);
	// Use all seeded ips as bootstrap ips (besides self_ip)
	let mut bootstrap_ips = vec![];
	for (_id, ip) in self.iter() {
	    if ip.clone() != self.ip.clone() {
		bootstrap_ips.push(ip.clone());
	    }
	}
	Box::pin(async move {
	    // Fanout requests to the bootstrap ips for version information
	    debug!("fanning requests to bootstrap ips");
	    let v = client::fanout(bootstrap_ips, Request::Version(Version { id, ip })).await;
	    BootstrapResult { responses: v }
	})
    }
}

//-- Update the peers when a succesful bootstrap quorum is obtained

#[derive(Debug, Clone, Serialize, Deserialize, Message)]
#[rtype(result = "Updated")]
struct UpdatePeers {
    responses: Vec<Response>,
}

#[derive(Debug, Clone, Serialize, Deserialize, MessageResponse)]
struct Updated {
    updates: Vec<(Id, SocketAddr)>,
    bootstrapped: bool,
}

impl Handler<UpdatePeers> for View {
    type Result = Updated;

    fn handle(&mut self, msg: UpdatePeers, _ctx: &mut Context<Self>) -> Self::Result {
	// Update the view with successful responses
	let mut updates = vec![];
	for response in msg.responses.iter() {
	    match response {
		Response::VersionAck(VersionAck { ip, peer_list }) => {
		    let peer_id = util::id_from_ip(&ip);
		    if self.insert_update(peer_id.clone(), ip.clone()) {
			updates.push((peer_id.clone(), ip.clone()));
		    }
		    for (peer_id, peer_ip) in peer_list {
			if self.insert_update(peer_id.clone(), peer_ip.clone()) {
			    updates.push((peer_id.clone(), peer_ip.clone()));
			}
		    }
		},
		// FIXME: Other responses are invalid #nosec
		_ => (),
	    }
	}
	let bootstrapped = if msg.responses.len() >= BOOTSTRAP_QUORUM {
	    true
	} else {
	    false
	};
	Updated { updates, bootstrapped }
    }
}

//-- Sample a random peer from the view

#[derive(Debug, Clone, Serialize, Deserialize, Message)]
#[rtype(result = "SampleResult")]
pub struct SampleOne;

#[derive(Debug, Clone, Serialize, Deserialize, MessageResponse)]
pub struct SampleResult {
    pub sample: Vec<(Id, SocketAddr)>,
}

impl Handler<SampleOne> for View {
    type Result = SampleResult;

    fn handle(&mut self, _msg: SampleOne, _ctx: &mut Context<Self>) -> Self::Result {
	let sample = self.sample_k(1);
	info!("sample = {:?}", sample.clone());
	SampleResult { sample }
    }
}

//-- Retry to bootstrap until a quorum is reached

pub async fn bootstrap(self_id: Id, view: Addr<View>, ice: Addr<Ice>) {
    let mut i = 3;
    loop {
	let BootstrapResult { responses } = view.send(Bootstrap {}).await.unwrap();
	let lim = responses.len();
	if lim > 0 {
	    let Updated { bootstrapped, .. } = view.send(UpdatePeers {
		responses: responses.clone(),
	    }).await.unwrap();
	    if bootstrapped {
		// Once a quorum has been established the `ice`
		// reservoir is bootstrapped with the peers in `view`.
		info!("obtained bootstrap quorum");
		let PeersResult { peers } = view.send(GetPeers).await.unwrap();
		if let Bootstrapped = ice.send(ice::Bootstrap { peers }).await.unwrap() {
		    break;
		}
	    }
	}
	let duration = tokio::time::Duration::from_millis(1000) * i;
	actix::clock::sleep(duration).await;
	i += 1;
    }
}
