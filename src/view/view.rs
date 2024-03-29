use super::sampleable_map::SampleableMap;

use crate::client::{ClientRequest, ClientResponse};
use crate::colored::Colorize;
use crate::ice::{self, Ice};
use crate::protocol::{Request, Response};
use crate::version::{Version, VersionAck};
use crate::zfx_id::Id;
use crate::{Error, Result};

use tracing::{debug, info};

use actix::{Actor, Addr, Context, Handler, Recipient};
use actix::{ActorFutureExt, ResponseActFuture};

use std::collections::HashSet;
use std::net::SocketAddr;

const PEER_LIST_MAX: usize = 3;
const BOOTSTRAP_QUORUM: usize = 2;

/// The view contains the most up to date set of peer metadata.
#[derive(Debug)]
pub struct View {
    /// The client used to make external requests.
    sender: Recipient<ClientRequest>,
    /// Node IP address
    ip: SocketAddr,
    /// Node Id
    node_id: Id,
    /// A map of peers for bootstrapping this node
    peers: SampleableMap<Id, SocketAddr>,
    /// A set of peers for bootstrapping this node
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
    /// Create new instance with empty `peers`.
    ///
    /// ## Parameters:
    /// * `sender` - the client for making external requests
    /// * `ip` - node IP address
    /// * `node_id` - node Id
    pub fn new(sender: Recipient<ClientRequest>, ip: SocketAddr, node_id: Id) -> Self {
        Self { sender, ip, node_id, peers: SampleableMap::new(), peer_list: HashSet::new() }
    }

    /// Add `peers` to the current `View`
    pub fn init(&mut self, peers: Vec<(Id, SocketAddr)>) {
        for (id, ip) in peers.iter() {
            if let None = self.insert(id.clone(), ip.clone()) {
                debug!("inserted <id: {:?}, ip: {:?}>", id.clone(), ip.clone());
            }
            if self.peer_list.len() < PEER_LIST_MAX {
                if !self.peer_list.contains(&(id.clone(), ip.clone())) {
                    debug!("inserting <id: {:?}, ip: {:?}> in peer list", id.clone(), ip.clone());
                    self.peer_list.insert((id.clone(), ip.clone()));
                }
            }
        }
    }

    /// Add a new peer to the `View` if it doesn't exist.
    ///
    /// Returns whether the element was updated or not (if the element was missing)
    pub fn insert_update(&mut self, id: Id, ip: SocketAddr) -> bool {
        if id == self.node_id {
            return false;
        }
        match self.insert(id, ip.clone()) {
            Some(_) => false,
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

    /// Get random `k`-peers
    pub fn sample_k(&mut self, k: usize) -> Vec<(Id, SocketAddr)> {
        if self.len() >= k {
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
        let id = msg.id.clone();
        let _ = self.insert_update(id, ip);

        // Fetch the peer list
        let mut peer_vec = vec![];
        for peer in self.peer_list.iter().cloned() {
            peer_vec.push(peer);
        }
        VersionAck { ip: self.ip.clone(), id: self.node_id.clone(), peer_list: peer_vec }
    }
}

/// Request for getting a list of nodes from the [View]
#[derive(Debug, Clone, Serialize, Deserialize, Message)]
#[rtype(result = "PeersResult")]
pub struct GetPeers;

/// Response to [GetPeers] with a list of nodes from the [View]
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

/// Request from [View] to bootstrap other nodes from the list of `peers`.
#[derive(Debug, Clone, Serialize, Deserialize, Message)]
#[rtype(result = "Result<BootstrapResult>")]
pub struct Bootstrap;

/// Response to [Bootstrap] having responses from each node from the list of `peers`.
#[derive(Debug, Clone, Serialize, Deserialize, MessageResponse)]
pub struct BootstrapResult {
    responses: Vec<Response>,
}

impl Handler<Bootstrap> for View {
    type Result = ResponseActFuture<Self, Result<BootstrapResult>>;

    fn handle(&mut self, _msg: Bootstrap, _ctx: &mut Context<Self>) -> Self::Result {
        let ip = self.ip.clone();
        let id = self.node_id;
        // Use all seeded ips as bootstrap ips (besides self_ip)
        let mut bootstrap_peers = vec![];
        for (id, ip) in self.iter() {
            if ip.clone() != self.ip.clone() {
                bootstrap_peers.push((id.clone(), ip.clone()));
            }
        }

        // Fanout requests to the bootstrap seeds
        let send_to_client = self.sender.send(ClientRequest::Fanout {
            peers: bootstrap_peers.clone(),
            request: Request::Version(Version { id, ip }),
        });
        // Wrap the future so that subsequent chained handlers can access the actor
        let send_to_client = actix::fut::wrap_future::<_, Self>(send_to_client);

        let handle_response = send_to_client.map(move |result, _actor, _ctx| match result {
            Ok(ClientResponse::Fanout(responses)) => Ok(BootstrapResult { responses }),
            Ok(_) => Err(Error::InvalidResponse),
            Err(e) => Err(Error::Actix(e)),
        });

        Box::pin(handle_response)
    }
}

/// Request to update the peers in [View] when a successful bootstrap quorum is obtained.
#[derive(Debug, Clone, Serialize, Deserialize, Message)]
#[rtype(result = "Updated")]
struct UpdatePeers {
    responses: Vec<Response>,
}

/// Response to [UpdatePeers] with a list of newly added nodes and
/// an indicator whether the current node is bootstrapped.
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
                Response::VersionAck(VersionAck { ip, id: peer_id, peer_list }) => {
                    if self.insert_update(peer_id.clone(), ip.clone()) {
                        updates.push((peer_id.clone(), ip.clone()));
                    }
                    for (peer_id, peer_ip) in peer_list {
                        if self.insert_update(peer_id.clone(), peer_ip.clone()) {
                            updates.push((peer_id.clone(), peer_ip.clone()));
                        }
                    }
                }
                // FIXME: Other responses are invalid #nosec
                _ => (),
            }
        }
        let bootstrapped = msg.responses.len() >= BOOTSTRAP_QUORUM;
        Updated { updates, bootstrapped }
    }
}

/// Sample random `k`-peers from the view.
#[derive(Debug, Clone, Serialize, Deserialize, Message)]
#[rtype(result = "SampleResult")]
pub struct SampleK {
    pub k: usize,
}

/// Response to [SampleK] with a list of nodes for sampling.
#[derive(Debug, Clone, Serialize, Deserialize, MessageResponse)]
pub struct SampleResult {
    pub sample: Vec<(Id, SocketAddr)>,
}
impl Handler<SampleK> for View {
    type Result = SampleResult;

    fn handle(&mut self, msg: SampleK, _ctx: &mut Context<Self>) -> Self::Result {
        let sample = self.sample_k(msg.k);
        debug!("sample (k: {:?}) = {:?}", msg.k, sample.clone());
        SampleResult { sample }
    }
}

/// Retry to bootstrap until the quorum is reached.
///
/// ## Parameters:
/// * `view` - address of [View] actor
/// * `ice` - address of [Ice][crate::ice::Ice] actor
pub async fn bootstrap(view: Addr<View>, ice: Addr<Ice>) {
    let mut i = 3;
    loop {
        let BootstrapResult { responses } = view.send(Bootstrap {}).await.unwrap().unwrap();
        let lim = responses.len();
        if lim > 0 {
            let Updated { bootstrapped, .. } =
                view.send(UpdatePeers { responses: responses.clone() }).await.unwrap();
            if bootstrapped {
                // Once a quorum has been established the `ice`
                // reservoir is bootstrapped with the peers in `view`.
                info!("[{}] obtained bootstrap quorum {}", "view".green(), "✓".green());
                let PeersResult { peers } = view.send(GetPeers).await.unwrap();
                if let ice::Bootstrapped(true) = ice.send(ice::Bootstrap { peers }).await.unwrap() {
                    break;
                }
            }
        }
        let duration = tokio::time::Duration::from_millis(1000) * i;
        actix::clock::sleep(duration).await;
        i += 1;
    }
}
