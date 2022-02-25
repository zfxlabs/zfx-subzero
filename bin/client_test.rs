use zfx_subzero::alpha::transfer::TransferOperation;
use zfx_subzero::client;
use zfx_subzero::protocol::{Request, Response};
use zfx_subzero::sleet;
use zfx_subzero::sleet::GenerateTxAck;
use zfx_subzero::tls;
use zfx_subzero::Result;

use ed25519_dalek::Keypair;
use std::net::SocketAddr;
use std::path::Path;
use std::time::Duration;

use tokio;
use tracing::info;
use tracing_subscriber;

use clap::{value_t, App, Arg};

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt().compact().with_max_level(tracing::Level::DEBUG).init();

    let matches = App::new("zfx-subzero")
        .version("0.1")
        .author("zero.fx labs ltd.")
        .about("Generates a transaction and sends it to `sleet`")
        .arg(
            Arg::with_name("peer-ip")
                .short("ip")
                .long("peer-ip")
                .value_name("PEER_IP")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("keypair")
                .short("kp")
                .long("keypair")
                .value_name("KEYPAIR_HEX")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("cell-hash")
                .short("h")
                .long("cell-hash")
                .value_name("CELL_HASH")
                .takes_value(true),
        )
        .arg(Arg::with_name("use-tls").long("use-tls").required(false))
        .arg(
            Arg::with_name("cert-path")
                .short("c")
                .long("cert-path")
                .value_name("CERT_PATH")
                .requires("use-tls")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("pk-path")
                .short("p")
                .long("priv-key-path")
                .value_name("PK_PATH")
                .requires("use-tls")
                .takes_value(true),
        )
        .arg(Arg::with_name("loop").short("l").long("loop").value_name("N").takes_value(true))
        .get_matches();

    // The peer to be contacted
    let peer_ip = value_t!(matches.value_of("peer-ip"), SocketAddr).unwrap_or_else(|e| e.exit());
    // The keypair that owns the `txhash` for spending
    let keypair = value_t!(matches.value_of("keypair"), String).unwrap_or_else(|e| e.exit());
    // The root `cell-hash` to spend
    let cell_hash = value_t!(matches.value_of("cell-hash"), String).unwrap_or_else(|e| e.exit());
    let n = value_t!(matches.value_of("loop"), u64).unwrap_or(1);
    let use_tls = matches.is_present("use-tls");

    // TCP/TLS setup
    let upgrader = if use_tls {
        let (cert, key) =
            tls::certificate::get_node_cert(Path::new("test.crt"), Path::new("test.key")).unwrap();
        let upgraders = tls::upgrader::tls_upgraders(&cert, &key);
        upgraders.client
    } else {
        tls::upgrader::TcpUpgrader::new()
    };

    // Reconstruct the keypair
    let keypair_bytes = hex::decode(keypair).unwrap();
    let keypair = Keypair::from_bytes(&keypair_bytes).unwrap();
    let encoded = bincode::serialize(&keypair.public).unwrap();
    let pkh = blake3::hash(&encoded).as_bytes().clone();

    let cell_hash_vec = hex::decode(cell_hash).unwrap();
    let mut cell_hash_bytes = [0u8; 32];
    for i in 0..32 {
        cell_hash_bytes[i] = cell_hash_vec[i];
    }

    for amount in 0..n {
        for retry in 1..11 {
            match client::oneshot(
                peer_ip,
                Request::GetCell(sleet::GetCell { cell_hash: cell_hash_bytes.clone() }),
                upgrader.clone(),
            )
            .await?
            {
                Some(Response::CellAck(sleet::CellAck { cell: Some(cell_in) })) => {
                    info!("spendable:\n{}\n", cell_in.clone());
                    let transfer_op = TransferOperation::new(
                        cell_in.clone(),
                        pkh.clone(),
                        pkh.clone(),
                        amount + 1,
                    );
                    let transfer_tx = transfer_op.transfer(&keypair).unwrap();
                    cell_hash_bytes = transfer_tx.hash();
                    match client::oneshot(
                        peer_ip,
                        Request::GenerateTx(sleet::GenerateTx { cell: transfer_tx.clone() }),
                        upgrader.clone(),
                    )
                    .await?
                    {
                        Some(Response::GenerateTxAck(GenerateTxAck { cell_hash: Some(_hash) })) => {
                            // info!("Ack hash: {}", hex::encode(_hash))
                        }
                        other => panic!("Unexpected: {:?}", other),
                    }
                    // info!("sent tx:\n{:?}\n", tx.clone());
                    info!("new cell_hash: {}", hex::encode(&cell_hash_bytes));
                    tokio::time::sleep(Duration::from_millis(1000)).await;
                    break;
                }
                other => {
                    info!("retrying to fetch {} ({:?})", hex::encode(&cell_hash_bytes), other);
                    if retry == 10 {
                        panic!("too many retries");
                    }
                    tokio::time::sleep(Duration::from_secs(retry)).await;
                }
            }
        }
    }
    Ok(())
}
