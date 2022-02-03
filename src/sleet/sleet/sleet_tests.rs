//! Tests for Sleet

use super::*;

use crate::alpha::coinbase::CoinbaseOperation;
use crate::alpha::transfer::TransferOperation;
use crate::cell::Cell;

use actix::{Addr, ResponseFuture};
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

#[derive(Debug, Clone, MessageResponse)]
pub struct SleetStatus {
    known_txs: sled::Db,
    queried_txs: sled::Db,
    conflict_graph_len: usize,
    live_cells: HashMap<CellHash, Cell>,
    accepted_txs: HashSet<TxHash>,
    rejected_txs: HashSet<TxHash>,
    dag_len: usize,
    accepted_frontier: HashSet<TxHash>,
}
impl Handler<GetStatus> for Sleet {
    type Result = SleetStatus;

    fn handle(&mut self, _msg: GetStatus, _ctx: &mut Context<Self>) -> Self::Result {
        SleetStatus {
            known_txs: self.known_txs.clone(),
            queried_txs: self.queried_txs.clone(),
            conflict_graph_len: self.conflict_graph.len(),
            live_cells: self.live_cells.clone(),
            accepted_txs: self.accepted_txs.clone(),
            rejected_txs: self.rejected_txs.clone(),
            dag_len: self.dag.len(),
            accepted_frontier: self.get_accepted_frontier().unwrap(),
        }
    }
}

fn mock_validator_id() -> Id {
    Id::one()
}

async fn sleep_ms(m: u64) {
    tokio::time::sleep(std::time::Duration::from_millis(m)).await;
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

// Client substitute for answering `QueryTx` queries
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
async fn set_validator_response(client: Addr<DummyClient>, response: bool) {
    client.send(SetResponses { responses: vec![(mock_validator_id(), response)] }).await.unwrap();
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
    // tracing_subscriber::fmt().compact().with_max_level(tracing::Level::INFO).try_init();
    let mut client = DummyClient::new();
    client.responses = vec![(mock_validator_id(), true)];
    let sender = client.start();

    let hail_mock = HailMock::new();
    let receiver = hail_mock.start();

    let sleet = Sleet::new(sender.clone().recipient(), receiver.clone().recipient(), Id::zero());
    let sleet_addr = sleet.start();

    let mut csprng = OsRng {};
    let root_kp = Keypair::generate(&mut csprng);
    let genesis_tx = generate_coinbase(&root_kp, 10000);

    let live_committee = make_live_committee(vec![genesis_tx.clone()]);
    sleet_addr.send(live_committee).await.unwrap();

    (sleet_addr, sender, receiver, root_kp, genesis_tx)
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
    for i in 0..N {
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

    let SleetStatus { dag_len, conflict_graph_len, rejected_txs, accepted_frontier, .. } =
        sleet.send(GetStatus).await.unwrap();
    assert_eq!(accepted_frontier.len(), 1);
    assert_eq!(dag_len, BETA1 as usize);
    assert_eq!(conflict_graph_len, 500);
    assert_eq!(rejected_txs.len(), 0);
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
        println!("Accepted: {}", a);
    }
    let _ = sleet.send(DumpDAG).await.unwrap();

    let SleetStatus { dag_len, accepted_frontier, .. } = sleet.send(GetStatus).await.unwrap();
    assert_eq!(accepted_frontier.len(), 1);
    // TODO It's sometimes 11, sometimes 12, check it
    assert!(dag_len < 13);
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
    let hashes = sleet.send(GetCellHashes).await.unwrap();
    assert_eq!(hashes.ids.len(), N + 1);

    // Wait a bit for 'Hail' to receive the message
    sleep_ms(10).await;

    let accepted = hail.send(GetAcceptedCells).await.unwrap();
    println!("Accepted: {}", accepted.len());
    assert!(accepted.len() == 1);
    let SleetStatus { accepted_frontier, .. } = sleet.send(GetStatus).await.unwrap();
    assert_eq!(accepted_frontier.len(), 1);
    println!("Accepted: {:?}", accepted);
    assert!(accepted == vec![cell0]);
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
