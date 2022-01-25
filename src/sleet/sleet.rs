use crate::colored::Colorize;
use crate::zfx_id::Id;

use crate::alpha::types::{TxHash, Weight};
use crate::cell::types::CellHash;
use crate::cell::{Cell, CellIds};
use crate::client::Fanout;
use crate::graph::conflict_graph::ConflictGraph;
use crate::graph::DAG;
use crate::hail::AcceptedCells;
use crate::protocol::{Request, Response};
use crate::storage::tx as tx_storage;
use crate::util;

use super::tx::Tx;
use super::{Error, Result};

use tracing::{debug, error, info};

use actix::{Actor, AsyncContext, Context, Handler, Recipient};
use actix::{ActorFutureExt, ResponseActFuture};

use std::collections::{HashMap, HashSet};
use std::net::SocketAddr;

// Parent selection

const NPARENTS: usize = 3;

// Safety parameters

pub const ALPHA: f64 = 0.5;
pub const BETA1: u8 = 11;
pub const BETA2: u8 = 20;

/// Sleet is a consensus bearing `mempool` for transactions conflicting on spent inputs.
pub struct Sleet {
    /// The client used to make external requests.
    sender: Recipient<Fanout>,
    /// Connection to Hail
    hail_recipient: Recipient<AcceptedCells>,
    /// The identity of this validator.
    node_id: Id,
    /// The weighted validator set.
    committee: HashMap<Id, (SocketAddr, Weight)>,
    /// The set of all known transactions in storage.
    known_txs: sled::Db,
    /// The set of all queried transactions in storage.
    queried_txs: sled::Db,
    /// The graph of conflicting transactions (potentially multi-input).
    conflict_graph: ConflictGraph,
    /// A mapping of a cell hashes to unspent cells.
    live_cells: HashMap<CellHash, Cell>,
    /// The map contains transaction already accepted
    accepted_txs: HashSet<TxHash>,
    /// The consensus graph.
    dag: DAG<TxHash>,
}

impl Sleet {
    // Initialisation - FIXME: Temporary databases
    pub fn new(
        sender: Recipient<Fanout>,
        hail_recipient: Recipient<AcceptedCells>,
        node_id: Id,
    ) -> Self {
        Sleet {
            sender,
            hail_recipient,
            node_id,
            committee: HashMap::default(),
            known_txs: sled::Config::new().temporary(true).open().unwrap(),
            queried_txs: sled::Config::new().temporary(true).open().unwrap(),
            conflict_graph: ConflictGraph::new(CellIds::empty()),
            live_cells: HashMap::default(),
            accepted_txs: HashSet::new(),
            dag: DAG::new(),
        }
    }

    /// Called for all newly discovered transactions.
    /// Returns `true` if the transaction haven't been encountered before
    fn on_receive_tx(&mut self, sleet_tx: Tx) -> Result<bool> {
        if !tx_storage::is_known_tx(&self.known_txs, sleet_tx.hash()).unwrap() {
            self.insert(sleet_tx.clone())?;
            let _ = tx_storage::insert_tx(&self.known_txs, sleet_tx.clone());
            Ok(true)
        } else {
            // info!("[{}] received already known transaction {}", "sleet".cyan(), tx.clone());
            Ok(false)
        }
    }

    // Vertices

    pub fn insert(&mut self, tx: Tx) -> Result<()> {
        let cell = tx.cell.clone();
        self.conflict_graph.insert_cell(cell.clone())?;
        self.dag.insert_vx(tx.hash(), tx.parents.clone())?;
        Ok(())
    }

    // Branch preference

    /// Starts at some vertex and does a depth first search in order to compute whether
    /// the vertex is strongly preferred (by checking whether all its ancestry is
    /// preferred).
    pub fn is_strongly_preferred(&self, tx: TxHash) -> Result<bool> {
        for ancestor in self.dag.dfs(&tx) {
            if !self.conflict_graph.is_preferred(ancestor)? {
                return Ok(false);
            }
        }
        Ok(true)
    }

    // Adaptive Parent Selection

    /// Starts at the live edges (the leaf nodes) of the `DAG` and does a depth first
    /// search until `p` preferrential parents are accumulated (or none if there are
    /// none).
    pub fn select_parents(&self, p: usize) -> Result<Vec<TxHash>> {
        if self.dag.is_empty() {
            return Ok(vec![]);
        }
        let mut parents = vec![];
        let leaves = self.dag.leaves();
        for leaf in leaves {
            for elt in self.dag.dfs(&leaf) {
                if self.is_strongly_preferred(elt.clone())? {
                    parents.push(elt.clone());
                    if parents.len() >= p {
                        // Found `p` preferred parents.
                        break;
                    } else {
                        // Found a preferred parent for this leaf so skip.
                        continue;
                    }
                }
            }
        }
        Ok(parents)
    }

    // Ancestral Preference

    // The ancestral update updates the preferred path through the DAG every time a new
    // vertex is added.
    pub fn update_ancestral_preference(&mut self, root_txhash: TxHash) -> Result<()> {
        for tx_hash in self.dag.dfs(&root_txhash) {
            // conviction of T vs Pt.pref
            let pref = self.conflict_graph.get_preferred(&tx_hash)?;
            let d1 = self.dag.conviction(tx_hash.clone())?;
            let d2 = self.dag.conviction(pref)?;
            // update the conflict set at this tx
            self.conflict_graph.update_conflict_set(&tx_hash, d1, d2)?;
        }
        Ok(())
    }

    // Finality

    /// Checks whether the transaction `TxHash` is accepted as final.
    pub fn is_accepted_tx(&self, tx_hash: &TxHash) -> Result<bool> {
        if self.conflict_graph.is_singleton(tx_hash)?
            && self.conflict_graph.get_confidence(tx_hash)? >= BETA1
        {
            Ok(true)
        } else {
            if self.conflict_graph.get_confidence(tx_hash)? >= BETA2 {
                Ok(true)
            } else {
                Ok(false)
            }
        }
    }

    /// Checks whether the parent of the provided `TxHash` is final - note that we do not
    /// traverse all of the parents of the accepted parent, since a child transaction
    /// cannot be final if its parent is not also final.
    pub fn is_accepted(&self, initial_tx_hash: &TxHash) -> Result<bool> {
        let mut parent_accepted = true;
        match self.dag.get(initial_tx_hash) {
            Some(parents) => {
                for parent in parents.iter() {
                    if !self.is_accepted_tx(&parent)? {
                        parent_accepted = false;
                        break;
                    }
                }
            }
            None => return Err(Error::InvalidTxHash(initial_tx_hash.clone())),
        }
        if parent_accepted {
            self.is_accepted_tx(initial_tx_hash)
        } else {
            Ok(false)
        }
    }

    // Accepted Frontier

    /// The accepted frontier of the DAG is a depth-first-search on the leaves of the DAG
    /// up to a vertices considered final, collecting all the final nodes.
    pub fn get_accepted_frontier(&self) -> Result<Vec<TxHash>> {
        if self.dag.is_empty() {
            return Ok(vec![]);
        }
        let mut accepted_frontier = vec![];
        let leaves = self.dag.leaves();
        for leaf in leaves {
            for tx_hash in self.dag.dfs(&leaf) {
                if self.is_accepted(tx_hash)? {
                    accepted_frontier.push(tx_hash.clone());
                    break;
                }
            }
        }
        Ok(accepted_frontier)
    }

    /// Check if a transaction or one of its ancestors have become accepted
    pub fn compute_accepted_txs(&mut self, tx_hash: &TxHash) -> Result<Vec<TxHash>> {
        let mut new = vec![];
        for t in self.dag.dfs(tx_hash) {
            if !self.accepted_txs.contains(t) && self.is_accepted(t)? {
                new.push(t.clone());
                let _ = self.accepted_txs.insert(t.clone());
            }
        }
        Ok(new)
    }

    // Weighted sampling

    pub fn sample(&self, minimum_weight: Weight) -> Result<Vec<(Id, SocketAddr)>> {
        let mut validators = vec![];
        for (id, (ip, w)) in self.committee.iter() {
            validators.push((id.clone(), ip.clone(), w.clone()));
        }
        util::sample_weighted(minimum_weight, validators).ok_or(Error::InsufficientWeight)
    }
}

impl Actor for Sleet {
    type Context = Context<Self>;

    fn started(&mut self, _ctx: &mut Context<Self>) {
        debug!("started sleet");
    }
}

// When the committee is initialised in `alpha` or when it comes back online due to a
// `FaultyNetwork` message received in `alpha`, `sleet` is updated with the latest relevant
// chain state.

#[derive(Debug, Clone, Serialize, Deserialize, Message)]
#[rtype(result = "()")]
pub struct LiveCommittee {
    pub validators: HashMap<Id, (SocketAddr, f64)>,
    pub live_cells: HashMap<CellHash, Cell>,
}

impl Handler<LiveCommittee> for Sleet {
    type Result = ();

    fn handle(&mut self, msg: LiveCommittee, _ctx: &mut Context<Self>) -> Self::Result {
        // Build the list of available UTXOs
        let txs_len = format!("{:?}", msg.live_cells.len());
        info!(
            "\n{} received {} transactions containing spendable outputs",
            "[sleet]".cyan(),
            txs_len.cyan()
        );
        let mut cell_ids_set: CellIds = CellIds::empty();
        for (cell_hash, cell) in msg.live_cells.clone() {
            info!("{}", cell.clone());
            let cell_ids = CellIds::from_outputs(cell_hash.clone(), cell.outputs()).unwrap();
            cell_ids_set = cell_ids_set.union(&cell_ids).cloned().collect();
        }
        info!("");
        self.live_cells = msg.live_cells.clone();
        self.conflict_graph = ConflictGraph::new(cell_ids_set);

        let mut s: String = format!("<<{}>>\n", "sleet".cyan());
        for (id, (_, w)) in msg.validators.clone() {
            let id_s = format!("{:?}", id).yellow();
            let w_s = format!("{:?}", w).cyan();
            s = format!("{} ν = {} {} | {} {}\n", s, "⦑".magenta(), id_s, w_s, "⦒".magenta());
        }
        info!("{}", s);

        self.committee = msg.validators;
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Message)]
#[rtype(result = "()")]
pub struct QueryIncomplete {
    pub tx: Tx,
    pub acks: Vec<Response>,
}

impl Handler<QueryIncomplete> for Sleet {
    type Result = ();

    fn handle(&mut self, _msg: QueryIncomplete, _ctx: &mut Context<Self>) -> Self::Result {
        ()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Message)]
#[rtype(result = "()")]
pub struct QueryComplete {
    pub tx: Tx,
    pub acks: Vec<Response>,
}

impl Handler<QueryComplete> for Sleet {
    type Result = ();

    fn handle(&mut self, msg: QueryComplete, ctx: &mut Context<Self>) -> Self::Result {
        // FIXME: Verify that there are no duplicate ids
        let mut outcomes = vec![];
        for ack in msg.acks.iter() {
            match ack {
                Response::QueryTxAck(qtx_ack) => match self.committee.get(&qtx_ack.id) {
                    Some((_, w)) => outcomes.push((qtx_ack.id, w.clone(), qtx_ack.outcome)),
                    None => (),
                },
                // FIXME: Error
                _ => (),
            }
        }
        //   if yes: set_chit(tx, 1), update ancestral preferences
        if util::sum_outcomes(outcomes) > ALPHA {
            self.dag.set_chit(msg.tx.hash(), 1).unwrap();
            self.update_ancestral_preference(msg.tx.hash()).unwrap();
            info!("[{}] query complete, chit = 1", "sleet".cyan());
            // Let `sleet` know that you can now build on this tx
            let _ = self.live_cells.insert(msg.tx.cell.hash(), msg.tx.cell.clone());

            // The transaction or some of its ancestors may have become
            // accepted. Check this.
            let new_accepted = self.compute_accepted_txs(&msg.tx.hash());
            match new_accepted {
                Ok(new_accepted) => {
                    if !new_accepted.is_empty() {
                        ctx.notify(NewAccepted { tx_hashes: new_accepted });
                    }
                }
                Err(e) => {
                    // It's a bug if happens
                    panic!("[sleet] Error checking whether transaction is accepted: {}", e);
                }
            }
        }
        //   if no:  set_chit(tx, 0) -- happens in `insert_vx`
        tx_storage::insert_tx(&self.queried_txs, msg.tx.clone()).unwrap();
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Message)]
#[rtype(result = "()")]
pub struct NewAccepted {
    pub tx_hashes: Vec<TxHash>,
}
impl Handler<NewAccepted> for Sleet {
    type Result = ();

    fn handle(&mut self, msg: NewAccepted, _ctx: &mut Context<Self>) -> Self::Result {
        let mut cells = vec![];
        for tx_hash in msg.tx_hashes.iter().cloned() {
            // At this point we can be sure that the tx is known
            let (_, tx) = tx_storage::get_tx(&self.known_txs, tx_hash).unwrap();
            info!("[{}] transaction is accepted\n{}", "sleet".cyan(), tx.clone());
            cells.push(tx.cell);
        }
        let _ = self.hail_recipient.do_send(AcceptedCells { cells });
    }
}

// Instead of having an infinite loop as per the paper which receives and processes
// inbound unqueried transactions, we instead use the `Actor` and use `notify` whenever
// a fresh transaction is received - either externally in `GenerateTx` or as an internal
// consensus message via `QueryTx`.

#[derive(Debug, Clone, Serialize, Deserialize, Message)]
#[rtype(result = "Result<()>")]
pub struct FreshTx {
    pub tx: Tx,
}

impl Handler<FreshTx> for Sleet {
    type Result = ResponseActFuture<Self, Result<()>>;

    fn handle(&mut self, msg: FreshTx, _ctx: &mut Context<Self>) -> Self::Result {
        let validators = self.sample(ALPHA).unwrap();
        info!("[{}] Querying\n{}", "sleet".cyan(), msg.tx.clone());
        info!("[{}] sampled {:?}", "sleet".cyan(), validators.clone());
        let mut validator_ips = vec![];
        for (_, ip) in validators.iter() {
            validator_ips.push(ip.clone());
        }

        // Fanout queries to sampled validators
        let send_to_client = self.sender.send(Fanout {
            ips: validator_ips.clone(),
            request: Request::QueryTx(QueryTx { tx: msg.tx.clone() }),
        });

        // Wrap the future so that subsequent chained handlers can access te actor.
        let send_to_client = actix::fut::wrap_future::<_, Self>(send_to_client);

        let update_self = send_to_client.map(move |result, _actor, ctx| {
            match result {
                Ok(acks) => {
                    // If the length of responses is the same as the length of the sampled ips,
                    // then every peer responded.
                    if acks.len() == validator_ips.len() {
                        Ok(ctx.notify(QueryComplete { tx: msg.tx.clone(), acks }))
                    } else {
                        Ok(ctx.notify(QueryIncomplete { tx: msg.tx.clone(), acks }))
                    }
                }
                Err(e) => Err(Error::Actix(e)),
            }
        });

        Box::pin(update_self)
    }
}

// Allow clients to fetch transactions for testing.

#[derive(Debug, Clone, Serialize, Deserialize, Message)]
#[rtype(result = "CellAck")]
pub struct GetCell {
    pub cell_hash: CellHash,
}

#[derive(Debug, Clone, Serialize, Deserialize, MessageResponse)]
pub struct CellAck {
    pub cell: Option<Cell>,
}

impl Handler<GetCell> for Sleet {
    type Result = CellAck;

    fn handle(&mut self, msg: GetCell, _ctx: &mut Context<Self>) -> Self::Result {
        CellAck { cell: self.live_cells.get(&msg.cell_hash).map(|x| x.clone()) }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Message)]
#[rtype(result = "GenerateTxAck")]
pub struct GenerateTx {
    pub cell: Cell,
}

#[derive(Debug, Clone, Serialize, Deserialize, MessageResponse)]
pub struct GenerateTxAck {
    /// hash of applied transaction
    pub cell_hash: Option<CellHash>,
}

impl Handler<GenerateTx> for Sleet {
    type Result = GenerateTxAck;

    fn handle(&mut self, msg: GenerateTx, ctx: &mut Context<Self>) -> Self::Result {
        let parents = self.select_parents(NPARENTS).unwrap();
        let sleet_tx = Tx::new(parents, msg.cell.clone());
        info!("[{}] Generating new transaction\n{}", "sleet".cyan(), sleet_tx.clone());

        match self.on_receive_tx(sleet_tx.clone()) {
            Ok(true) => {
                ctx.notify(FreshTx { tx: sleet_tx });
                GenerateTxAck { cell_hash: Some(msg.cell.hash()) }
            }
            Ok(false) => GenerateTxAck { cell_hash: None },

            Err(e) => {
                error!(
                    "[{}] Couldn't insert new transaction\n{}:\n {}",
                    "sleet".cyan(),
                    sleet_tx,
                    e
                );
                GenerateTxAck { cell_hash: None }
            }
        }
    }
}

// Receiving transactions. The only difference between receiving transactions and receiving
// a transaction query is that any client should be able to send `sleet` a `GenerateTx`
// message, whereas only network validators should be able to perform a `QueryTx`.
//
// Otherwise the functionality is identical but `QueryTx` returns a consensus response -
// whether the transaction is strongly preferred or not.

#[derive(Debug, Clone, Serialize, Deserialize, Message)]
#[rtype(result = "QueryTxAck")]
pub struct QueryTx {
    pub tx: Tx,
}

#[derive(Debug, Clone, Serialize, Deserialize, MessageResponse)]
pub struct QueryTxAck {
    pub id: Id,
    pub tx_hash: TxHash,
    pub outcome: bool,
}

impl Handler<QueryTx> for Sleet {
    type Result = QueryTxAck;

    fn handle(&mut self, msg: QueryTx, ctx: &mut Context<Self>) -> Self::Result {
        info!("[{}] Received query for transaction {}", "sleet".cyan(), hex::encode(msg.tx.hash()));
        match self.on_receive_tx(msg.tx.clone()) {
            Ok(true) => ctx.notify(FreshTx { tx: msg.tx.clone() }),
            Ok(false) => (),
            Err(e) => {
                error!(
                    "[{}] Couldn't insert queried transaction {:?}: {}",
                    "sleet".cyan(),
                    msg.tx,
                    e
                );
            }
        }
        // FIXME: If we are in the middle of querying this transaction, wait until a
        // decision or a synchronous timebound is reached on attempts.
        match self.is_strongly_preferred(msg.tx.hash()) {
            Ok(outcome) => QueryTxAck { id: self.node_id, tx_hash: msg.tx.hash(), outcome },
            Err(e) => {
                error!("[{}] Missing ancestor of {}\n {}", "sleet".cyan(), msg.tx, e);
                // FIXME We're voting against the tx w/o having enough information
                QueryTxAck { id: self.node_id, tx_hash: msg.tx.hash(), outcome: false }
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Message)]
#[rtype(result = "CellHashes")]
pub struct GetCellHashes;

#[derive(Debug, Clone, Serialize, Deserialize, MessageResponse)]
pub struct CellHashes {
    pub ids: Vec<CellHash>,
}

impl Handler<GetCellHashes> for Sleet {
    type Result = CellHashes;

    fn handle(&mut self, _msg: GetCellHashes, _ctx: &mut Context<Self>) -> Self::Result {
        return CellHashes { ids: self.live_cells.keys().cloned().collect::<Vec<CellHash>>() };
    }
}

#[cfg(test)]
mod test {
    use super::*;

    use crate::alpha::coinbase::CoinbaseOperation;
    use crate::alpha::transfer::TransferOperation;
    use crate::cell::Cell;

    use actix::ResponseFuture;
    use ed25519_dalek::Keypair;
    use rand::rngs::OsRng;

    use std::convert::TryInto;

    fn generate_coinbase(keypair: &Keypair, amount: u64) -> Cell {
        let enc = bincode::serialize(&keypair.public).unwrap();
        let pkh = blake3::hash(&enc).as_bytes().clone();
        let coinbase_op = CoinbaseOperation::new(vec![
            (pkh.clone(), amount),
            (pkh.clone(), amount + 1),
            (pkh.clone(), amount + 2),
        ]);
        coinbase_op.try_into().unwrap()
    }

    fn generate_transfer(keypair: &Keypair, from: Cell, amount: u64) -> Cell {
        let enc = bincode::serialize(&keypair.public).unwrap();
        let pkh = blake3::hash(&enc).as_bytes().clone();
        let transfer_op = TransferOperation::new(from, pkh.clone(), pkh, amount);
        transfer_op.transfer(&keypair).unwrap()
    }

    fn mock_validator_id() -> Id {
        Id::one()
    }

    fn make_live_committee(cells: Vec<Cell>) -> LiveCommittee {
        let mut validators = HashMap::new();

        // We have one overweight validator for tests
        validators.insert(mock_validator_id(), ("0.0.0.0:1".parse().unwrap(), 0.7));
        let mut live_cells = HashMap::new();
        for c in cells {
            live_cells.insert(c.hash(), c.clone());
        }
        LiveCommittee { validators, live_cells }
    }

    struct DummyClient {
        pub responses: Vec<(Id, bool)>,
    }

    impl DummyClient {
        pub fn new() -> Self {
            Self { responses: vec![] }
        }
    }
    impl Actor for DummyClient {
        type Context = Context<Self>;

        fn started(&mut self, _ctx: &mut Context<Self>) {}
    }

    #[derive(Debug, Clone, Serialize, Deserialize, Message)]
    #[rtype(result = "()")]
    struct SetResponses {
        pub responses: Vec<(Id, bool)>,
    }
    impl Handler<SetResponses> for DummyClient {
        type Result = ();

        fn handle(
            &mut self,
            SetResponses { responses }: SetResponses,
            _ctx: &mut Context<Self>,
        ) -> Self::Result {
            self.responses = responses;
        }
    }
    impl Handler<Fanout> for DummyClient {
        type Result = ResponseFuture<Vec<Response>>;

        fn handle(
            &mut self,
            Fanout { ips: _, request }: Fanout,
            _ctx: &mut Context<Self>,
        ) -> Self::Result {
            let responses = self.responses.clone();
            Box::pin(async move {
                match request {
                    Request::QueryTx(QueryTx { tx }) => responses
                        .iter()
                        .map(|(id, outcome)| {
                            Response::QueryTxAck(QueryTxAck {
                                id: id.clone(),
                                tx_hash: tx.hash(),
                                outcome: outcome.clone(),
                            })
                        })
                        .collect(),
                    r => panic!("unexpected request: {:?}", r),
                }
            })
        }
    }

    struct HailMock {
        pub accepted: Vec<Cell>,
    }
    impl HailMock {
        pub fn new() -> Self {
            Self { accepted: vec![] }
        }
    }
    impl Actor for HailMock {
        type Context = Context<Self>;

        fn started(&mut self, _ctx: &mut Context<Self>) {}
    }

    impl Handler<AcceptedCells> for HailMock {
        type Result = ();

        fn handle(&mut self, msg: AcceptedCells, _ctx: &mut Context<Self>) -> Self::Result {
            self.accepted.extend_from_slice(&msg.cells[..])
        }
    }

    #[derive(Debug, Clone, Serialize, Deserialize, Message)]
    #[rtype(result = "Vec<Cell>")]
    struct GetAcceptedCells;

    impl Handler<GetAcceptedCells> for HailMock {
        type Result = Vec<Cell>;

        fn handle(&mut self, _msg: GetAcceptedCells, _ctx: &mut Context<Self>) -> Self::Result {
            self.accepted.clone()
        }
    }

    #[actix_rt::test]
    async fn smoke_test_sleet() {
        let mut client = DummyClient::new();
        client.responses = vec![(mock_validator_id(), true)];
        let sender = client.start();

        let hail_mock = HailMock::new();
        let receiver = hail_mock.start();

        let sleet = Sleet::new(sender.recipient(), receiver.clone().recipient(), Id::zero());
        let sleet_addr = sleet.start();

        let mut csprng = OsRng {};
        let root_kp = Keypair::generate(&mut csprng);
        let genesis_tx = generate_coinbase(&root_kp, 1000);

        let live_committee = make_live_committee(vec![genesis_tx.clone()]);
        sleet_addr.send(live_committee).await.unwrap();

        let cell = generate_transfer(&root_kp, genesis_tx.clone(), 1);
        let hash = cell.hash();
        sleet_addr.send(GenerateTx { cell }).await.unwrap();

        let hashes = sleet_addr.send(GetCellHashes).await.unwrap();
        assert_eq!(hashes.ids.len(), 2);
        assert!(hashes.ids.contains(&hash));

        let accepted = receiver.send(GetAcceptedCells).await.unwrap();
        assert!(accepted.is_empty());
    }

    #[actix_rt::test]
    async fn test_sleet_accept() {
        const MIN_TXS_NEEDED: usize = 8;
        let mut client = DummyClient::new();
        client.responses = vec![(mock_validator_id(), true)];
        let sender = client.start();

        let hail_mock = HailMock::new();
        let receiver = hail_mock.start();

        let sleet = Sleet::new(sender.recipient(), receiver.clone().recipient(), Id::zero());
        let sleet_addr = sleet.start();

        let mut csprng = OsRng {};
        let root_kp = Keypair::generate(&mut csprng);
        let genesis_tx = generate_coinbase(&root_kp, 1000);

        let live_committee = make_live_committee(vec![genesis_tx.clone()]);
        sleet_addr.send(live_committee).await.unwrap();

        let mut spend_cell = genesis_tx.clone();
        let mut cell0: Cell = genesis_tx.clone(); // value irrelevant, will be initialised later
        for i in 0..MIN_TXS_NEEDED {
            let cell = generate_transfer(&root_kp, spend_cell.clone(), 1 + i as u64);
            if i == 0 {
                cell0 = cell.clone();
            }
            sleet_addr.send(GenerateTx { cell: cell.clone() }).await.unwrap();
            spend_cell = cell;
        }
        let hashes = sleet_addr.send(GetCellHashes).await.unwrap();
        assert_eq!(hashes.ids.len(), MIN_TXS_NEEDED + 1);

        let accepted = receiver.send(GetAcceptedCells).await.unwrap();
        println!("Accepted: {:?}", accepted);
        assert!(accepted.len() == 1);
        assert!(accepted == vec![cell0]);
    }

    #[actix_rt::test]
    async fn test_sleet_dont_accept() {
        const N: usize = 30;
        let mut client = DummyClient::new();
        // all transactions will be voted against,
        // none will be accepted
        client.responses = vec![(mock_validator_id(), false)];
        let sender = client.start();

        let hail_mock = HailMock::new();
        let receiver = hail_mock.start();

        let sleet = Sleet::new(sender.recipient(), receiver.clone().recipient(), Id::zero());
        let sleet_addr = sleet.start();

        let mut csprng = OsRng {};
        let root_kp = Keypair::generate(&mut csprng);
        let genesis_tx = generate_coinbase(&root_kp, 1000);

        let live_committee = make_live_committee(vec![genesis_tx.clone()]);
        sleet_addr.send(live_committee).await.unwrap();

        let mut spend_cell = genesis_tx.clone();
        for i in 0..N {
            let cell = generate_transfer(&root_kp, spend_cell.clone(), 1 + i as u64);
            sleet_addr.send(GenerateTx { cell: cell.clone() }).await.unwrap();
            spend_cell = cell;
        }

        // The cells won't be added to `live_cells`. TODO Is this correct?
        let hashes = sleet_addr.send(GetCellHashes).await.unwrap();
        assert_eq!(hashes.ids.len(), 1);

        let accepted = receiver.send(GetAcceptedCells).await.unwrap();
        println!("Accepted: {:?}", accepted);
        assert!(accepted.is_empty());
    }

    #[actix_rt::test]
    async fn test_strongly_preferred() {
        let client = DummyClient::new();
        let sender = client.start();
        let hail_mock = HailMock::new();
        let receiver = hail_mock.start();

        let mut csprng = OsRng {};
        let root_kp = Keypair::generate(&mut csprng);

        let genesis_tx = generate_coinbase(&root_kp, 1000);
        let genesis_cell_ids =
            CellIds::from_outputs(genesis_tx.hash(), genesis_tx.outputs()).unwrap();
        let mut sleet = Sleet::new(sender.recipient(), receiver.recipient(), Id::zero());
        sleet.conflict_graph = ConflictGraph::new(genesis_cell_ids);

        // Generate a genesis set of coins
        let stx1 = Tx::new(vec![], generate_transfer(&root_kp, genesis_tx.clone(), 1000));
        let stx2 = Tx::new(vec![], generate_transfer(&root_kp, genesis_tx.clone(), 1001));
        let stx3 = Tx::new(vec![], generate_transfer(&root_kp, genesis_tx.clone(), 1002));

        // Check that parent selection works with an empty DAG.
        let v_empty: Vec<TxHash> = vec![];
        assert_eq!(sleet.select_parents(3).unwrap(), v_empty.clone());

        // Insert new vertices into the DAG.
        sleet.insert(stx1.clone()).unwrap();
        sleet.insert(stx2.clone()).unwrap();
        sleet.insert(stx3.clone()).unwrap();

        // Coinbase transactions will all conflict, since `tx1` was inserted first it will
        // be the only preferred parent.
        assert_eq!(sleet.select_parents(3).unwrap(), vec![stx1.cell.hash(),]);
    }
}
