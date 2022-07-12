use criterion::measurement::WallTime;
use criterion::{
    black_box, criterion_group, criterion_main, BenchmarkGroup, BenchmarkId, Criterion, Throughput,
};
use ed25519_dalek::Keypair;
use std::convert::TryInto;

use std::hash::Hash;
use zfx_subzero::alpha::coinbase::CoinbaseOperation;
use zfx_subzero::alpha::state::State;
use zfx_subzero::cell::{Cell, CellIds};
use zfx_subzero::graph::conflict_graph::ConflictGraph;

use zfx_subzero::alpha::transfer::TransferOperation;
use zfx_subzero::alpha::types::TxHash;
use zfx_subzero::benchmark_test_util::{
    build_blocks, create_keys, create_n_cells, get_transactions_from_db, insert_into_dag,
    insert_into_dependency_graph, make_db_inserts,
};

pub fn run_dag_benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("dag_benchmark");
    let iterations = vec![100, 1000, 10000];

    insert_into_dag_benchmark(&mut group, iterations.clone());
    remove_from_dag_benchmark(&mut group, iterations.clone());
    conviction_dag_benchmark(&mut group, vec![10]);
    get_ancestors_in_dag_benchmark(&mut group, vec![10]);
    bfs_in_dag_benchmark(&mut group, iterations.clone());

    group.finish();
}

pub fn run_conflict_graph_benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("conflict_graph_benchmark");
    let iterations = vec![100, 1000, 10000];

    insert_into_conflict_graph_benchmark(&mut group, iterations.clone());
    remove_from_conflict_graph_benchmark(&mut group, iterations.clone());
    accept_in_conflict_graph_benchmark(&mut group, iterations.clone());

    group.finish();
}

pub fn run_dependency_graph_benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("dependency_graph_benchmark");
    let iterations = vec![100, 1000, 10000];

    insert_into_dependency_graph_benchmark(&mut group, iterations.clone());
    topological_dependency_graph_benchmark(&mut group, iterations.clone());

    group.finish();
}

pub fn run_state_benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("state_benchmark");
    for i in [1u64, 10u64, 100u64, 1000u64].iter() {
        group.bench_with_input(BenchmarkId::new("state", i), i, |b, i| {
            b.iter(|| {
                let mut state = State::new();

                for block in build_blocks(*i) {
                    state = state.apply(block).unwrap();
                }
            })
        });
    }
    group.finish();
}

pub fn run_transfer_benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("transfer_benchmark");
    for i in [100, 1000, 10000].iter() {
        group.throughput(Throughput::Elements(*i as u64));

        group.bench_with_input(BenchmarkId::new("transfer", i), i, |b, i| {
            b.iter(|| {
                let (keypair_1, _, pub_key_1, pub_key_2) = create_keys();

                let coinbase_op = CoinbaseOperation::new(vec![(pub_key_1.clone(), 10000000)]);
                let mut transfer_tx = coinbase_op.try_into().unwrap();

                for _ in 1..*i {
                    let transfer_op = TransferOperation::new(
                        transfer_tx,
                        pub_key_2.clone(),
                        pub_key_1.clone(),
                        1,
                    );
                    transfer_tx = transfer_op.transfer(&keypair_1).unwrap();
                }
            })
        });
    }
    group.finish();
}

pub fn run_tx_db_benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("tx_db_benchmark");
    let iterations = vec![100, 1000, 10000];

    insert_tx_into_db_benchmark(&mut group, iterations.clone());
    get_transactions_from_db_benchmark(&mut group, iterations.clone());

    group.finish();
}

fn insert_into_dag_benchmark(group: &mut BenchmarkGroup<WallTime>, iterations: Vec<u64>) {
    for i in iterations.iter() {
        let cell_hashes = create_n_cells(*i).iter().map(|c| c.hash()).collect::<Vec<TxHash>>();

        group.throughput(Throughput::Elements(*i as u64));
        group.bench_with_input(BenchmarkId::new("insert_into_dag", i), i, |b, i| {
            b.iter(|| insert_into_dag(cell_hashes.clone()))
        });
    }
}

fn remove_from_dag_benchmark(group: &mut BenchmarkGroup<WallTime>, iterations: Vec<u64>) {
    for i in iterations.iter() {
        let tx_hashes = create_n_cells(*i).iter().map(|c| c.hash()).collect::<Vec<TxHash>>();
        let mut dag = insert_into_dag(tx_hashes.clone());

        group.throughput(Throughput::Elements(*i as u64));
        group.bench_with_input(BenchmarkId::new("remove_from_dag", i), i, |b, i| {
            b.iter(|| {
                for tx_hash in tx_hashes.clone() {
                    dag.remove_vx(&tx_hash);
                }
            })
        });
    }
}

fn conviction_dag_benchmark(group: &mut BenchmarkGroup<WallTime>, iterations: Vec<u64>) {
    for i in iterations.iter() {
        let tx_hashes = create_n_cells(*i).iter().map(|c| c.hash()).collect::<Vec<TxHash>>();
        let mut dag = insert_into_dag(tx_hashes.clone());
        for tx_hash in tx_hashes.clone() {
            dag.set_chit(tx_hash, 1);
        }

        group.throughput(Throughput::Elements(*i as u64));
        group.bench_with_input(BenchmarkId::new("conviction_dag", i), i, |b, i| {
            b.iter(|| {
                for tx_hash in tx_hashes.clone() {
                    dag.conviction(tx_hash);
                }
            })
        });
    }
}

fn get_ancestors_in_dag_benchmark(group: &mut BenchmarkGroup<WallTime>, iterations: Vec<u64>) {
    for i in iterations.iter() {
        let tx_hashes = create_n_cells(*i).iter().map(|c| c.hash()).collect::<Vec<TxHash>>();
        let mut dag = insert_into_dag(tx_hashes.clone());

        group.throughput(Throughput::Elements(*i as u64));
        group.bench_with_input(BenchmarkId::new("get_ancestors_in_dag", i), i, |b, i| {
            b.iter(|| {
                for tx_hash in tx_hashes.clone() {
                    dag.get_ancestors(&tx_hash);
                }
            })
        });
    }
}

fn bfs_in_dag_benchmark(group: &mut BenchmarkGroup<WallTime>, iterations: Vec<u64>) {
    for i in iterations.iter() {
        let tx_hashes = create_n_cells(*i).iter().map(|c| c.hash()).collect::<Vec<TxHash>>();
        let mut dag = insert_into_dag(tx_hashes.clone());

        group.throughput(Throughput::Elements(*i as u64));
        group.bench_with_input(BenchmarkId::new("bfs_in_dag", i), i, |b, i| {
            b.iter(|| {
                for tx_hash in tx_hashes.clone() {
                    dag.bfs(tx_hash);
                }
            })
        });
    }
}

fn insert_into_conflict_graph_benchmark(
    group: &mut BenchmarkGroup<WallTime>,
    iterations: Vec<u64>,
) {
    for i in iterations.iter() {
        let (cells, mut dh) = create_conflict_graph(*i);
        group.throughput(Throughput::Elements(*i as u64));
        group.bench_with_input(BenchmarkId::new("insert_into_conflict_graph", i), i, |b, i| {
            b.iter(|| {
                for cell in cells.clone() {
                    dh.insert_cell(cell);
                }
            })
        });
    }
}

fn remove_from_conflict_graph_benchmark(
    group: &mut BenchmarkGroup<WallTime>,
    iterations: Vec<u64>,
) {
    for i in iterations.iter() {
        let (cells, mut dh) = create_conflict_graph(*i);
        for cell in cells.clone() {
            dh.insert_cell(cell);
        }
        group.throughput(Throughput::Elements(*i as u64));
        group.bench_with_input(BenchmarkId::new("remove_from_conflict_graph", i), i, |b, i| {
            b.iter(|| {
                for cell in cells.clone() {
                    dh.remove_cell(&cell.hash());
                }
            })
        });
    }
}

fn accept_in_conflict_graph_benchmark(group: &mut BenchmarkGroup<WallTime>, iterations: Vec<u64>) {
    for i in iterations.iter() {
        let (cells, mut dh) = create_conflict_graph(*i);
        for cell in cells.clone() {
            dh.insert_cell(cell);
        }
        group.throughput(Throughput::Elements(*i as u64));
        group.bench_with_input(BenchmarkId::new("accept_in_conflict_graph", i), i, |b, i| {
            b.iter(|| {
                for cell in cells.clone() {
                    dh.accept_cell(cell);
                }
            })
        });
    }
}

fn insert_into_dependency_graph_benchmark(
    group: &mut BenchmarkGroup<WallTime>,
    iterations: Vec<u64>,
) {
    for i in iterations.iter() {
        group.throughput(Throughput::Elements(*i as u64));

        let cells = create_n_cells(*i);
        group.bench_with_input(BenchmarkId::new("insert_into_dependency_graph", i), i, |b, i| {
            b.iter(|| insert_into_dependency_graph(cells.clone()))
        });
    }
}

fn topological_dependency_graph_benchmark(
    group: &mut BenchmarkGroup<WallTime>,
    iterations: Vec<u64>,
) {
    for i in iterations.iter() {
        group.throughput(Throughput::Elements(*i as u64));

        let graph = insert_into_dependency_graph(create_n_cells(*i));
        group.bench_with_input(BenchmarkId::new("topological_dependency_graph", i), i, |b, i| {
            b.iter(|| graph.topological())
        });
    }
}

fn insert_tx_into_db_benchmark(group: &mut BenchmarkGroup<WallTime>, iterations: Vec<u64>) {
    for i in iterations.iter() {
        group.throughput(Throughput::Elements(*i as u64));

        group.bench_with_input(BenchmarkId::new("insert_tx_into_db", i), i, |b, i| {
            b.iter(|| {
                let mut tx_db = sled::Config::new().temporary(true).open().unwrap();
                make_db_inserts(&tx_db, *i);
            })
        });
    }
}

fn get_transactions_from_db_benchmark(group: &mut BenchmarkGroup<WallTime>, iterations: Vec<u64>) {
    for i in iterations.iter() {
        group.throughput(Throughput::Elements(*i as u64));

        let mut tx_db = sled::Config::new().temporary(true).open().unwrap();
        let tx_hashes = make_db_inserts(&tx_db, *i);
        group.bench_with_input(BenchmarkId::new("get_transactions_from_db", i), i, |b, i| {
            b.iter(|| get_transactions_from_db(&mut tx_db, tx_hashes.clone()))
        });
    }
}

fn create_conflict_graph(n: u64) -> (Vec<Cell>, ConflictGraph) {
    let cells = create_n_cells(n);
    let genesis_output_cell_ids =
        CellIds::from_outputs(cells[0].hash(), cells[0].outputs()).unwrap();

    (cells, ConflictGraph::new(genesis_output_cell_ids.clone()))
}

criterion_group!(
    benches,
    run_conflict_graph_benchmark,
    run_dag_benchmark,
    run_dependency_graph_benchmark,
    run_state_benchmark,
    run_transfer_benchmark,
    run_tx_db_benchmark
);
criterion_main!(benches);
