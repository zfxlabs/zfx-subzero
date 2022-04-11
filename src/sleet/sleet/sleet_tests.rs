//! Tests for Sleet

use super::*;

use crate::alpha::coinbase::CoinbaseOperation;
use crate::alpha::transfer::TransferOperation;
use crate::cell::Cell;

use actix::{Addr, ResponseFuture};
use ed25519_dalek::Keypair;
use rand::rngs::OsRng;

use std::convert::TryInto;
use std::time::Instant;

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
    match transfer_op.transfer(&keypair) {
        Ok(tr) => tr,
        Err(e) => panic!("{}", e),
    }
}

fn new_pkh() -> [u8; 32] {
    let mut csprng = OsRng {};
    let recv_k = Keypair::generate(&mut csprng);
    let enc2 = bincode::serialize(&recv_k.public).unwrap();
    blake3::hash(&enc2).as_bytes().clone()
}

fn generate_transfer_whith_recipient(
    keypair: &Keypair,
    from: Cell,
    recipient: [u8; 32],
    amount: u64,
) -> Cell {
    let enc = bincode::serialize(&keypair.public).unwrap();
    let pkh = blake3::hash(&enc).as_bytes().clone();
    let transfer_op = TransferOperation::new(from, recipient, pkh, amount);
    match transfer_op.transfer(&keypair) {
        Ok(tr) => tr,
        Err(e) => panic!("{}", e),
    }
}

// For debugging: the output can be fed to `dot` to draw the graph
#[derive(Debug, Clone, Serialize, Deserialize, Message)]
#[rtype(result = "()")]
pub struct DumpDAG;

impl Handler<DumpDAG> for Sleet {
    type Result = ();

    fn handle(&mut self, _msg: DumpDAG, _ctx: &mut Context<Self>) -> Self::Result {
        println!("\n\ndigraph G {{\n");
        for (v, edges) in self.dag.iter() {
            for e in edges.iter() {
                println!("\"{}\" -> \"{}\"", hex::encode(v), hex::encode(e));
            }
        }
        println!("}}\n");
    }
}

/// Get as much of Sleet's state as possible
#[derive(Debug, Clone, Message)]
#[rtype(result = "SleetStatus")]
pub struct GetStatus;

#[allow(unused)] // Some fields are never read currently
#[derive(Debug, Clone, MessageResponse)]
pub struct SleetStatus {
    known_txs: sled::Db,
    conflict_graph_len: usize,
    live_cells: HashMap<CellHash, Cell>,
    accepted_txs: HashSet<TxHash>,
    dag_len: usize,
    accepted_frontier: HashSet<TxHash>,
}
impl Handler<GetStatus> for Sleet {
    type Result = SleetStatus;

    fn handle(&mut self, _msg: GetStatus, _ctx: &mut Context<Self>) -> Self::Result {
        SleetStatus {
            known_txs: self.known_txs.clone(),
            conflict_graph_len: self.conflict_graph.len(),
            live_cells: self.live_cells.clone(),
            accepted_txs: self.accepted_txs.clone(),
            dag_len: self.dag.len(),
            accepted_frontier: self.accepted_frontier.clone(),
        }
    }
}

fn mock_validator_id() -> Id {
    Id::one()
}

fn mock_ip() -> SocketAddr {
    "0.0.0.0:1".parse().unwrap()
}

async fn sleep_ms(m: u64) {
    tokio::time::sleep(std::time::Duration::from_millis(m)).await;
}

fn make_live_committee(cells: Vec<Cell>) -> LiveCommittee {
    let mut validators = HashMap::new();

    // We have one overweight validator for tests
    validators.insert(mock_validator_id(), (mock_ip(), 0.7));
    let mut live_cells = HashMap::new();
    for c in cells {
        live_cells.insert(c.hash(), c.clone());
    }
    LiveCommittee { validators, live_cells }
}

struct DummyClient {
    // For responding to `QueryTx`
    pub responses: Vec<(Id, bool)>,
    // For answering `GetAncestors` messages
    pub ancestors: Vec<Tx>,
}

// Client substitute for answering `QueryTx` queries
impl DummyClient {
    pub fn new() -> Self {
        Self { responses: vec![], ancestors: vec![] }
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
async fn set_validator_response(client: Addr<DummyClient>, response: bool) {
    client.send(SetResponses { responses: vec![(mock_validator_id(), response)] }).await.unwrap();
}

#[derive(Debug, Clone, Serialize, Deserialize, Message)]
#[rtype(result = "()")]
struct SetAncestors {
    pub ancestors: Vec<Tx>,
}
impl Handler<SetAncestors> for DummyClient {
    type Result = ();

    fn handle(
        &mut self,
        SetAncestors { ancestors }: SetAncestors,
        _ctx: &mut Context<Self>,
    ) -> Self::Result {
        self.ancestors = ancestors;
    }
}
async fn set_ancestors(client: Addr<DummyClient>, ancestors: Vec<Tx>) {
    client.send(SetAncestors { ancestors }).await.unwrap();
}

impl Handler<ClientRequest> for DummyClient {
    type Result = ResponseFuture<ClientResponse>;

    fn handle(&mut self, msg: ClientRequest, _ctx: &mut Context<Self>) -> Self::Result {
        let responses = self.responses.clone();
        match msg {
            ClientRequest::Fanout { peers: _, request } => Box::pin(async move {
                let r = match request {
                    Request::QueryTx(QueryTx { tx, .. }) => responses
                        .iter()
                        .map(|(id, outcome)| {
                            Response::QueryTxAck(QueryTxAck {
                                id: id.clone(),
                                tx_hash: tx.hash(),
                                outcome: outcome.clone(),
                            })
                        })
                        .collect(),
                    x => panic!("unexpected request: {:?}", x),
                };
                ClientResponse::Fanout(r)
            }),
            ClientRequest::Oneshot { id: _, ip: _, request } => {
                let ancestors = self.ancestors.clone();
                Box::pin(async move {
                    let r = match request {
                        Request::GetTxAncestors(GetTxAncestors { .. }) => {
                            println!("GetAncestors");
                            Response::TxAncestors(TxAncestors { ancestors })
                        }
                        x => panic!("unexpected request: {:?}", x),
                    };
                    ClientResponse::Oneshot(Some(r))
                })
            } // ClientRequest::Oneshot { ip: _, request: _ } => panic!("unexpected message"),
        }
    }
}

// Receives accepted transactions from Sleet and stores them in a vector
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

async fn start_test_env() -> (Addr<Sleet>, Addr<DummyClient>, Addr<HailMock>, Keypair, Cell) {
    // Uncomment to see Sleet's logs
    // let _ = tracing_subscriber::fmt().compact().with_max_level(tracing::Level::INFO).try_init();
    let mut client = DummyClient::new();
    client.responses = vec![(mock_validator_id(), true)];
    let sender = client.start();

    let hail_mock = HailMock::new();
    let receiver = hail_mock.start();

    let sleet =
        Sleet::new(sender.clone().recipient(), receiver.clone().recipient(), Id::zero(), mock_ip());
    let sleet_addr = sleet.start();

    let mut csprng = OsRng {};
    let root_kp = Keypair::generate(&mut csprng);
    let genesis_tx = generate_coinbase(&root_kp, 10000);

    let live_committee = make_live_committee(vec![genesis_tx.clone()]);
    sleet_addr.send(live_committee).await.unwrap();

    (sleet_addr, sender, receiver, root_kp, genesis_tx)
}

async fn start_test_env_with_two_sleet_actors(
) -> (Addr<Sleet>, Addr<Sleet>, Addr<DummyClient>, Addr<HailMock>, Keypair, Cell) {
    // Uncomment to see Sleet's logs
    // tracing_subscriber::fmt().compact().with_max_level(tracing::Level::INFO).try_init();

    let (sleet_addr, client, hail, root_kp, genesis_tx) = start_test_env().await;

    let sleet2 =
        Sleet::new(client.clone().recipient(), hail.clone().recipient(), Id::one(), mock_ip());
    let sleet_addr2 = sleet2.start();

    let live_committee = make_live_committee(vec![genesis_tx.clone()]);
    sleet_addr2.send(live_committee).await.unwrap();

    (sleet_addr, sleet_addr2, client, hail, root_kp, genesis_tx)
}

async fn start_test_env_with_two_sleet_actors_and_two_cells(
) -> (Addr<Sleet>, Addr<Sleet>, Addr<DummyClient>, Addr<HailMock>, Keypair, Vec<Cell>) {
    // Uncomment to see Sleet's logs
    // let _ = tracing_subscriber::fmt().compact().with_max_level(tracing::Level::INFO).try_init();
    let mut client = DummyClient::new();
    client.responses = vec![(mock_validator_id(), true)];
    let sender = client.start();

    let hail_mock = HailMock::new();
    let receiver = hail_mock.start();

    let sleet =
        Sleet::new(sender.clone().recipient(), receiver.clone().recipient(), Id::zero(), mock_ip());
    let sleet_addr = sleet.start();

    let mut csprng = OsRng {};
    let root_kp = Keypair::generate(&mut csprng);
    let genesis_tx1 = generate_coinbase(&root_kp, 10000);
    let genesis_tx2 = generate_coinbase(&root_kp, 20000);

    let live_committee = make_live_committee(vec![genesis_tx1.clone(), genesis_tx2.clone()]);
    sleet_addr.send(live_committee.clone()).await.unwrap();

    let sleet2 =
        Sleet::new(sender.clone().recipient(), receiver.clone().recipient(), Id::one(), mock_ip());
    let sleet_addr2 = sleet2.start();

    sleet_addr2.send(live_committee).await.unwrap();

    (sleet_addr, sleet_addr2, sender, receiver, root_kp, vec![genesis_tx1, genesis_tx2])
}

#[actix_rt::test]
async fn smoke_test_sleet() {
    let (sleet, _client, hail, root_kp, genesis_tx) = start_test_env().await;

    let cell = generate_transfer(&root_kp, genesis_tx.clone(), 1);
    let hash = cell.hash();
    sleet.send(GenerateTx { cell }).await.unwrap();

    let hashes = sleet.send(GetCellHashes).await.unwrap();
    assert_eq!(hashes.ids.len(), 2);
    assert!(hashes.ids.contains(&hash));

    let accepted = hail.send(GetAcceptedCells).await.unwrap();
    assert!(accepted.is_empty());
}

#[actix_rt::test]
async fn test_duplicate_tx() {
    let (sleet, _client, hail, root_kp, genesis_tx) = start_test_env().await;

    let cell = generate_transfer(&root_kp, genesis_tx.clone(), 1);
    let hash = cell.hash();
    match sleet.send(GenerateTx { cell: cell.clone() }).await.unwrap() {
        GenerateTxAck { cell_hash: Some(h) } => assert!(hash == h),
        other => panic!("unexpected: {:?}", other),
    }

    // Trying the same tx a second time
    match sleet.send(GenerateTx { cell }).await.unwrap() {
        GenerateTxAck { cell_hash: None } => (),
        other => panic!("unexpected: {:?}", other),
    }

    let hashes = sleet.send(GetCellHashes).await.unwrap();
    assert_eq!(hashes.ids.len(), 2);
    assert!(hashes.ids.contains(&hash));

    let accepted = hail.send(GetAcceptedCells).await.unwrap();
    assert!(accepted.is_empty());
}

#[actix_rt::test]
async fn test_coinbase_tx() {
    let (sleet, _client, _hail, root_kp, _genesis_tx) = start_test_env().await;

    let cell = generate_coinbase(&root_kp, 1);
    let hash = cell.hash();

    // Trying to insert a coinbase tx
    match sleet.send(GenerateTx { cell }).await.unwrap() {
        GenerateTxAck { cell_hash: None } => (),
        other => panic!("unexpected: {:?}", other),
    }

    let hashes = sleet.send(GetCellHashes).await.unwrap();
    assert_eq!(hashes.ids.len(), 1);
    assert!(!hashes.ids.contains(&hash));
}

#[actix_rt::test]
async fn test_spend_nonexistent_funds() {
    let (sleet, _client, _hail, root_kp, _genesis_tx) = start_test_env().await;

    let unknown_coinbase = generate_coinbase(&root_kp, 1);
    let bad_cell = generate_transfer(&root_kp, unknown_coinbase, 1);

    match sleet.send(GenerateTx { cell: bad_cell }).await.unwrap() {
        GenerateTxAck { cell_hash: None } => (),
        other => panic!("unexpected: {:?}", other),
    }
}

#[actix_rt::test]
async fn test_sleet_accept_one() {
    const MIN_CHILDREN_NEEDED: usize = BETA1 as usize;

    let (sleet, _client, hail, root_kp, genesis_tx) = start_test_env().await;

    let mut spend_cell = genesis_tx.clone();
    let mut cell0: Cell = genesis_tx.clone(); // value irrelevant, will be initialised later
    for i in 0..MIN_CHILDREN_NEEDED {
        let cell = generate_transfer(&root_kp, spend_cell.clone(), 1 + i as u64);
        if i == 0 {
            cell0 = cell.clone();
        }
        println!("Cell: {}", cell.clone());

        sleet.send(GenerateTx { cell: cell.clone() }).await.unwrap();
        spend_cell = cell;
    }
    let hashes = sleet.send(GetCellHashes).await.unwrap();
    assert_eq!(hashes.ids.len(), MIN_CHILDREN_NEEDED + 1);
    // let _ = sleet.send(DumpDAG).await.unwrap();

    let accepted = hail.send(GetAcceptedCells).await.unwrap();
    println!("Accepted: {:?}", accepted);
    assert!(accepted.len() == 1);
    assert!(accepted == vec![cell0]);
}

#[actix_rt::test]
async fn test_sleet_accept_many() {
    const N: usize = 500;

    let (sleet, _client, hail, root_kp, genesis_tx) = start_test_env().await;
    let addr = new_pkh();

    let mut spend_cell = genesis_tx.clone();
    for _ in 0..N {
        let cell = generate_transfer_whith_recipient(&root_kp, spend_cell.clone(), addr, 1);
        println!("Cell: {}", cell.clone());

        sleet.send(GenerateTx { cell: cell.clone() }).await.unwrap();
        spend_cell = cell;
    }
    let hashes = sleet.send(GetCellHashes).await.unwrap();
    assert_eq!(hashes.ids.len(), N + 1);
    // let _ = sleet.send(DumpDAG).await.unwrap();

    let accepted = hail.send(GetAcceptedCells).await.unwrap();
    assert!(accepted.len() == N + 1 - BETA1 as usize);

    let SleetStatus { dag_len, conflict_graph_len, accepted_frontier, .. } =
        sleet.send(GetStatus).await.unwrap();
    assert_eq!(accepted_frontier.len(), 1);
    assert_eq!(dag_len, BETA1 as usize);
    assert_eq!(conflict_graph_len, 500);
}

#[actix_rt::test]
async fn test_sleet_accept_with_conflict() {
    const CHILDREN_NEEDED: usize = BETA2 as usize;
    let (sleet, client, hail, root_kp, genesis_tx) = start_test_env().await;

    let first_cell = generate_transfer(&root_kp, genesis_tx.clone(), 100);
    println!("First cell: {} {}", hex::encode(first_cell.hash()), first_cell.clone());
    sleet.send(GenerateTx { cell: first_cell.clone() }).await.unwrap();

    // Spends the same outputs, will conflict with `first_cell`
    let conflicting_cell = generate_transfer(&root_kp, genesis_tx.clone(), 42);
    // Make sure the mock validator votes against
    set_validator_response(client.clone(), false).await;
    sleet.send(GenerateTx { cell: conflicting_cell.clone() }).await.unwrap();
    println!(
        "Conflicting cell: {} {}",
        hex::encode(conflicting_cell.hash()),
        conflicting_cell.clone()
    );
    sleep_ms(100).await;
    set_validator_response(client.clone(), true).await;

    let mut spend_cell = first_cell.clone();
    for i in 0..CHILDREN_NEEDED {
        println!("Spending: {}\n {}", hex::encode(spend_cell.hash()), spend_cell.clone());
        let cell = generate_transfer(&root_kp, spend_cell.clone(), 1 + i as u64);
        sleet.send(GenerateTx { cell: cell.clone() }).await.unwrap();
        println!("Cell: {}", cell.clone());
        spend_cell = cell;
    }
    let hashes = sleet.send(GetCellHashes).await.unwrap();
    // + 2: `genesis_tx` and `first_cell`, the voted down tx won't be added to `live_cells`
    assert_eq!(hashes.ids.len(), CHILDREN_NEEDED + 2);

    // Wait a bit for 'Hail' to receive the message
    sleep_ms(10).await;

    let accepted = hail.send(GetAcceptedCells).await.unwrap();
    for a in accepted.iter() {
        println!("Accepted: {}", hex::encode(a.hash()));
    }
    let _ = sleet.send(DumpDAG).await.unwrap();

    let SleetStatus { dag_len, accepted_frontier, .. } = sleet.send(GetStatus).await.unwrap();
    let accepted_frontier_len = accepted_frontier.len();
    println!("dag_len: {}", dag_len);
    println!("accepted_frontier_len: {}", accepted_frontier_len);
    assert!(dag_len == 10 + accepted_frontier_len);
    // The conflicting transaction is accepted after BETA2 queries,
    // and its non-conflictiong children after BETA1
    assert_eq!(accepted.len(), 11);
    assert!(accepted.contains(&first_cell));
}

#[actix_rt::test]
async fn test_sleet_dont_accept() {
    const N: usize = 30;
    let (sleet, client, hail, root_kp, genesis_tx) = start_test_env().await;

    // all transactions will be voted against,
    // none will be accepted
    set_validator_response(client, false).await;

    let mut spend_cell = genesis_tx.clone();
    for i in 0..N {
        let cell = generate_transfer(&root_kp, spend_cell.clone(), 1 + i as u64);
        sleet.send(GenerateTx { cell: cell.clone() }).await.unwrap();
        spend_cell = cell;
    }

    // The cells won't be added to `live_cells`, as all of them were voted against
    let hashes = sleet.send(GetCellHashes).await.unwrap();
    assert_eq!(hashes.ids.len(), 1);

    let accepted = hail.send(GetAcceptedCells).await.unwrap();
    println!("Accepted: {:?}", accepted);
    assert!(accepted.is_empty());
}

#[actix_rt::test]
async fn test_sleet_many_conflicts() {
    const N: usize = BETA2 as usize;

    let (sleet, client, hail, root_kp, genesis_tx) = start_test_env().await;

    let mut spend_cell = genesis_tx.clone();
    let mut cell0: Cell = genesis_tx.clone(); // value irrelevant, will be initialised later
    for i in 0..N {
        let cell = generate_transfer(&root_kp, spend_cell.clone(), 1 + i as u64);
        if i == 0 {
            cell0 = cell.clone();
        }
        // println!("Cell: {}", cell.clone());
        sleet.send(GenerateTx { cell: cell.clone() }).await.unwrap();

        set_validator_response(client.clone(), false).await;
        let conflict_cell = generate_transfer(&root_kp, spend_cell.clone(), 50 + 1 + i as u64);
        // println!("conflicting cell: {}", conflict_cell.clone());
        sleet.send(GenerateTx { cell: conflict_cell.clone() }).await.unwrap();
        sleep_ms(10).await;
        set_validator_response(client.clone(), true).await;

        spend_cell = cell;
    }
    // let _ = sleet.send(DumpDAG).await.unwrap();

    // Since confidence resets for ancestors when querying a conflicting child,
    // we need more children for getting the first transaction finalised
    for i in 0..N {
        let cell = generate_transfer(&root_kp, spend_cell.clone(), 100 + 1 + i as u64);
        sleet.send(GenerateTx { cell: cell.clone() }).await.unwrap();
        spend_cell = cell;
    }
    let hashes = sleet.send(GetCellHashes).await.unwrap();
    assert_eq!(hashes.ids.len(), 2 * N + 1);

    // Wait a bit for 'Hail' to receive the message
    sleep_ms(10).await;

    let accepted = hail.send(GetAcceptedCells).await.unwrap();
    println!("Accepted: {}", accepted.len());
    assert!(accepted.len() >= 1);
    let SleetStatus { accepted_frontier, .. } = sleet.send(GetStatus).await.unwrap();
    println!("Accepted frontier: {}", accepted_frontier.len());
    assert!(accepted_frontier.len() >= 1);
    // println!("Accepted: {:?}", accepted);
    assert!(accepted.contains(&cell0));
    // let _ = sleet.send(DumpDAG).await.unwrap();
}

#[actix_rt::test]
async fn test_sleet_tx_no_parents() {
    let (sleet1, sleet2, _client, _hail, root_kp, genesis_tx) =
        start_test_env_with_two_sleet_actors().await;
    let cell = genesis_tx.clone();

    let cell1 = generate_transfer(&root_kp, cell.clone(), 1);
    sleet1.send(GenerateTx { cell: cell1.clone() }).await.unwrap();
    let cell2 = generate_transfer(&root_kp, cell1.clone(), 2);
    sleet1.send(GenerateTx { cell: cell2.clone() }).await.unwrap();

    // Get tx from `sleet1`, and check if `cell1` is a parent
    let SleetStatus { known_txs, .. } = sleet1.send(GetStatus).await.unwrap();
    let (_, tx) = tx_storage::get_tx(&known_txs, cell2.hash()).unwrap();
    assert!(tx.parents.contains(&cell1.hash()));

    // Query at sleet2 and wait till it times out
    let now = Instant::now();
    let QueryTxAck { outcome, .. } =
        sleet2.send(QueryTx { id: Id::zero(), ip: mock_ip(), tx }).await.unwrap();
    assert!(!outcome);
    let elapsed = now.elapsed().as_millis();
    assert!(elapsed >= QUERY_RESPONSE_TIMEOUT_MS as u128);
}

#[actix_rt::test]
async fn test_sleet_tx_late_parents() {
    let (sleet1, sleet2, _client, _hail, root_kp, genesis_tx) =
        start_test_env_with_two_sleet_actors().await;
    let cell = genesis_tx.clone();

    let cell1 = generate_transfer(&root_kp, cell.clone(), 1);
    sleet1.send(GenerateTx { cell: cell1.clone() }).await.unwrap();
    let cell2 = generate_transfer(&root_kp, cell1.clone(), 2);
    sleet1.send(GenerateTx { cell: cell2.clone() }).await.unwrap();

    // Get tx from `sleet1`, and check if `cell1` is a parent
    let SleetStatus { known_txs, .. } = sleet1.send(GetStatus).await.unwrap();
    let (_, tx1) = tx_storage::get_tx(&known_txs, cell1.hash()).unwrap();
    let (_, tx2) = tx_storage::get_tx(&known_txs, cell2.hash()).unwrap();
    assert!(tx2.parents.contains(&tx1.hash()));

    let (tx, rx) = oneshot::channel();
    let sleet_clone = sleet2.clone();
    tokio::spawn(async move {
        let QueryTxAck { outcome, .. } =
            sleet_clone.send(QueryTx { id: Id::zero(), ip: mock_ip(), tx: tx1 }).await.unwrap();
        assert!(outcome);
        let _ = tx.send(outcome);
    });

    sleep_ms(1000).await;
    let QueryTxAck { outcome, .. } =
        sleet2.send(QueryTx { id: Id::zero(), ip: mock_ip(), tx: tx2 }).await.unwrap();
    assert!(outcome);
    assert!(rx.await.unwrap());
}

#[actix_rt::test]
async fn test_sleet_tx_two_late_parents() {
    let (sleet1, sleet2, _client, _hail, root_kp, genesis_tx) =
        start_test_env_with_two_sleet_actors().await;
    let cell = genesis_tx.clone();

    let cell1 = generate_transfer(&root_kp, cell.clone(), 1);
    sleet1.send(GenerateTx { cell: cell1.clone() }).await.unwrap();
    let cell2 = generate_transfer(&root_kp, cell1.clone(), 2);
    sleet1.send(GenerateTx { cell: cell2.clone() }).await.unwrap();
    let cell3 = generate_transfer(&root_kp, cell2.clone(), 3);
    sleet1.send(GenerateTx { cell: cell3.clone() }).await.unwrap();

    // Get tx from `sleet1`, and check if `cell1` is a parent
    let SleetStatus { known_txs, .. } = sleet1.send(GetStatus).await.unwrap();
    let (_, tx1) = tx_storage::get_tx(&known_txs, cell1.hash()).unwrap();
    let (_, tx2) = tx_storage::get_tx(&known_txs, cell2.hash()).unwrap();
    let (_, tx3) = tx_storage::get_tx(&known_txs, cell3.hash()).unwrap();
    assert!(tx2.parents.contains(&tx1.hash()));
    assert!(tx3.parents.contains(&tx2.hash()));

    let (tx, rx3) = oneshot::channel();
    let sleet_clone = sleet2.clone();
    tokio::spawn(async move {
        let QueryTxAck { outcome, .. } =
            sleet_clone.send(QueryTx { id: Id::zero(), ip: mock_ip(), tx: tx1 }).await.unwrap();
        assert!(outcome);
        let _ = tx.send(outcome);
    });

    let (tx, rx2) = oneshot::channel();
    let sleet_clone = sleet2.clone();
    tokio::spawn(async move {
        let QueryTxAck { outcome, .. } =
            sleet_clone.send(QueryTx { id: Id::zero(), ip: mock_ip(), tx: tx2 }).await.unwrap();
        assert!(outcome);
        let _ = tx.send(outcome);
    });

    sleep_ms(1000).await;
    let QueryTxAck { outcome: outcome1, .. } =
        sleet2.send(QueryTx { id: Id::zero(), ip: mock_ip(), tx: tx3 }).await.unwrap();
    assert!(outcome1);
    assert!(rx3.await.unwrap());
    assert!(rx2.await.unwrap());
}

#[actix_rt::test]
async fn test_sleet_tx_missing_parent() {
    let (sleet1, sleet2, _client, _hail, root_kp, genesis_tx) =
        start_test_env_with_two_sleet_actors().await;
    let cell = genesis_tx.clone();

    let cell1 = generate_transfer(&root_kp, cell.clone(), 1);
    sleet1.send(GenerateTx { cell: cell1.clone() }).await.unwrap();
    let cell2 = generate_transfer(&root_kp, cell1.clone(), 2);
    sleet1.send(GenerateTx { cell: cell2.clone() }).await.unwrap();
    let cell3 = generate_transfer(&root_kp, cell2.clone(), 3);
    sleet1.send(GenerateTx { cell: cell3.clone() }).await.unwrap();

    // Get tx from `sleet1`, and check if `cell1` is a parent
    let SleetStatus { known_txs, .. } = sleet1.send(GetStatus).await.unwrap();
    let (_, tx1) = tx_storage::get_tx(&known_txs, cell1.hash()).unwrap();
    let (_, tx2) = tx_storage::get_tx(&known_txs, cell2.hash()).unwrap();
    let (_, tx3) = tx_storage::get_tx(&known_txs, cell3.hash()).unwrap();
    assert!(tx2.parents.contains(&tx1.hash()));
    assert!(tx3.parents.contains(&tx2.hash()));

    let (tx, rx1) = oneshot::channel();
    let sleet_clone = sleet2.clone();
    tokio::spawn(async move {
        let QueryTxAck { outcome, .. } =
            sleet_clone.send(QueryTx { id: Id::zero(), ip: mock_ip(), tx: tx1 }).await.unwrap();
        assert!(outcome);
        let _ = tx.send(outcome);
    });

    // `tx2` will be missing, this causes the query for `tx3` to time out

    sleep_ms(1000).await;
    let QueryTxAck { outcome: outcome3, .. } =
        sleet2.send(QueryTx { id: Id::zero(), ip: mock_ip(), tx: tx3 }).await.unwrap();
    assert!(!outcome3);
    assert!(rx1.await.unwrap());
}

#[actix_rt::test]
async fn test_sleet_get_single_ancestor() {
    let (sleet1, sleet2, client, _hail, root_kp, genesis_tx) =
        start_test_env_with_two_sleet_actors().await;
    let cell = genesis_tx.clone();

    let cell1 = generate_transfer(&root_kp, cell.clone(), 1);
    sleet1.send(GenerateTx { cell: cell1.clone() }).await.unwrap();
    let cell2 = generate_transfer(&root_kp, cell1.clone(), 2);
    sleet1.send(GenerateTx { cell: cell2.clone() }).await.unwrap();

    // Get tx from `sleet1`, and check if `cell1` is a parent
    let SleetStatus { known_txs, .. } = sleet1.send(GetStatus).await.unwrap();
    let (_, tx1) = tx_storage::get_tx(&known_txs, cell1.hash()).unwrap();
    let (_, tx2) = tx_storage::get_tx(&known_txs, cell2.hash()).unwrap();
    assert!(tx2.parents.contains(&tx1.hash()));

    set_ancestors(client, vec![tx1.clone()]).await;

    let QueryTxAck { outcome, .. } =
        sleet2.send(QueryTx { id: Id::zero(), ip: mock_ip(), tx: tx2 }).await.unwrap();
    assert!(outcome);
}

#[actix_rt::test]
async fn test_sleet_get_wrong_ancestor() {
    let (sleet1, sleet2, client, _hail, root_kp, genesis_tx) =
        start_test_env_with_two_sleet_actors().await;
    let cell = genesis_tx.clone();

    let cell1 = generate_transfer(&root_kp, cell.clone(), 1);
    sleet1.send(GenerateTx { cell: cell1.clone() }).await.unwrap();
    let cell2 = generate_transfer(&root_kp, cell1.clone(), 2);
    sleet1.send(GenerateTx { cell: cell2.clone() }).await.unwrap();

    // Get tx from `sleet1`, and check if `cell1` is a parent
    let SleetStatus { known_txs, .. } = sleet1.send(GetStatus).await.unwrap();
    let (_, tx1) = tx_storage::get_tx(&known_txs, cell1.hash()).unwrap();
    let (_, tx2) = tx_storage::get_tx(&known_txs, cell2.hash()).unwrap();
    assert!(tx2.parents.contains(&tx1.hash()));

    // Answer with the same tx, not the expected ancestry
    set_ancestors(client, vec![tx2.clone()]).await;

    let QueryTxAck { outcome, .. } =
        sleet2.send(QueryTx { id: Id::zero(), ip: mock_ip(), tx: tx2 }).await.unwrap();
    assert!(!outcome);
}

#[actix_rt::test]
async fn test_sleet_remove_children_of_rejected() {
    let (sleet1, sleet2, client, _hail, root_kp, genesis_txs) =
        start_test_env_with_two_sleet_actors_and_two_cells().await;

    // Make sure that both `sleet1` and `sleet2` know about `cell1`
    let cell1 = generate_transfer(&root_kp, genesis_txs[0].clone(), 1000);
    sleet1.send(GenerateTx { cell: cell1.clone() }).await.unwrap();

    let SleetStatus { known_txs, .. } = sleet1.send(GetStatus).await.unwrap();
    let (_, tx1) = tx_storage::get_tx(&known_txs, cell1.hash()).unwrap();

    let QueryTxAck { outcome, .. } =
        sleet2.send(QueryTx { id: Id::zero(), ip: mock_ip(), tx: tx1 }).await.unwrap();
    assert!(outcome);

    // `cell2` and `cell2_rogue` conflict; `cell3` doesn't conflict
    // with any other transaction, but it will be a child of `cell2_rogue` in `sleet2`
    let cell2 = generate_transfer(&root_kp, cell1.clone(), 1);
    sleet1.send(GenerateTx { cell: cell2.clone() }).await.unwrap();

    let cell2_rogue = generate_transfer(&root_kp, cell1.clone(), 2);
    let cell3 = generate_transfer(&root_kp, genesis_txs[1].clone(), 1);

    sleet2.send(GenerateTx { cell: cell2_rogue.clone() }).await.unwrap();
    sleet2.send(GenerateTx { cell: cell3.clone() }).await.unwrap();

    let SleetStatus { known_txs, .. } = sleet2.send(GetStatus).await.unwrap();
    let (_, tx2_rogue) = tx_storage::get_tx(&known_txs, cell2_rogue.hash()).unwrap();
    let (_, tx3) = tx_storage::get_tx(&known_txs, cell3.hash()).unwrap();
    assert!(tx3.parents.contains(&tx2_rogue.hash()));

    // Add `tx2_rogue` and `tx3` to `sleet1`; neither will be preferred
    set_validator_response(client.clone(), false).await;
    let QueryTxAck { outcome, .. } =
        sleet1.send(QueryTx { id: Id::zero(), ip: mock_ip(), tx: tx2_rogue }).await.unwrap();
    assert!(!outcome);
    let QueryTxAck { outcome, .. } =
        sleet1.send(QueryTx { id: Id::zero(), ip: mock_ip(), tx: tx3 }).await.unwrap();
    assert!(!outcome);
    set_validator_response(client, true).await;

    // let _ = sleet1.send(DumpDAG).await.unwrap();

    // Send `BETA2` transactions to make `tx2` accepted
    let mut spend_cell = cell2.clone();
    const CHILDREN_NEEDED: usize = BETA2 as usize;
    for i in 0..CHILDREN_NEEDED {
        let cell = generate_transfer(&root_kp, spend_cell.clone(), 30 + i as u64);
        sleet1.send(GenerateTx { cell: cell.clone() }).await.unwrap();
        spend_cell = cell;
    }

    // let _ = sleet1.send(DumpDAG).await.unwrap();

    let SleetStatus { known_txs, .. } = sleet1.send(GetStatus).await.unwrap();
    let (_, tx2) = tx_storage::get_tx(&known_txs, cell2.hash()).unwrap();
    let (_, tx2_rogue) = tx_storage::get_tx(&known_txs, cell2_rogue.hash()).unwrap();
    let (_, tx3) = tx_storage::get_tx(&known_txs, cell3.hash()).unwrap();
    assert!(tx3.parents.contains(&tx2_rogue.hash()));
    assert!(tx2.status == TxStatus::Accepted);
    assert!(tx2_rogue.status == TxStatus::Rejected);
    assert!(tx3.status == TxStatus::Removed);

    // Re-try `tx3`
    match sleet1.send(GenerateTx { cell: cell3.clone() }).await.unwrap() {
        GenerateTxAck { cell_hash: Some(_) } => (),
        GenerateTxAck { cell_hash: None } => panic!("re-issuing transaction failed"),
    }
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
    let genesis_cell_ids = CellIds::from_outputs(genesis_tx.hash(), genesis_tx.outputs()).unwrap();
    let mut sleet = Sleet::new(sender.recipient(), receiver.recipient(), Id::zero(), mock_ip());
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
