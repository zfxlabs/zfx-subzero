use zfx_subzero::chain::alpha::{Transaction, TransferTx};
use zfx_subzero::client;
use zfx_subzero::protocol::{Request, Response};
use zfx_subzero::sleet;
use zfx_subzero::version;
use zfx_subzero::zfx_id::Id;
use zfx_subzero::Result;

use ed25519_dalek::Keypair;
use std::net::SocketAddr;

use tracing::info;
use tracing_subscriber;

use clap::{value_t, App, Arg};

fn id_from_ip(ip: &SocketAddr) -> Id {
    Id::new(format!("{:?}", ip.clone()).as_bytes())
}

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
            Arg::with_name("txhash")
                .short("h")
                .long("txhash")
                .value_name("TX_HASH")
                .takes_value(true),
        )
        .get_matches();

    // The peer to be contacted
    let peer_ip = value_t!(matches.value_of("peer-ip"), SocketAddr).unwrap_or_else(|e| e.exit());
    // The keypair that owns the `txhash` for spending
    let keypair = value_t!(matches.value_of("keypair"), String).unwrap_or_else(|e| e.exit());
    // The root `txhash` to spend
    let txhash = value_t!(matches.value_of("txhash"), String).unwrap_or_else(|e| e.exit());

    // Reconstruct the keypair
    let keypair_bytes = hex::decode(keypair).unwrap();
    let keypair = Keypair::from_bytes(&keypair_bytes).unwrap();
    let encoded = bincode::serialize(&keypair.public).unwrap();
    let pkh = blake3::hash(&encoded).as_bytes().clone();

    let tx_hash_vec = hex::decode(txhash).unwrap();
    let mut tx_hash = [0u8; 32];
    let mut i = 0;
    for i in 0..32 {
        tx_hash[i] = tx_hash_vec[i];
    }

    if let Some(Response::TxAck(tx_ack)) =
        client::oneshot(peer_ip, Request::GetTx(sleet::GetTx { tx_hash: tx_hash.clone() })).await?
    {
        let inner_tx = tx_ack.tx.unwrap().inner();
        info!("spendable: {:?}", inner_tx);
        // Construct a new tx and send it to the mempool. Note that we use the `tx_hash` of
        // the `Transaction` rather than the inner `Tx` (maybe FIXME needs to be looked at).
        let transfer_tx = TransferTx::new(&keypair, tx_hash, inner_tx, pkh.clone(), pkh.clone(), 1);
        let tx = Transaction::TransferTx(transfer_tx);
        let _ = client::oneshot(peer_ip, Request::GenerateTx(sleet::GenerateTx { tx })).await?;
    }

    Ok(())
}
