use zfx_id::Id;
use zfx_subzero::Result;
use zfx_subzero::client;
use zfx_subzero::version;
use zfx_subzero::protocol::Request;

use tracing_subscriber;

use clap::{value_t, App, Arg};

use std::net::SocketAddr;

fn id_from_ip(ip: &SocketAddr) -> Id {
    Id::new(format!("{:?}", ip.clone()).as_bytes())
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .compact()
        .with_max_level(tracing::Level::DEBUG)
        .init();

    let matches = App::new("zfx-subzero")
        .version("0.1")
        .author("zero.fx labs ltd.")
        .about("Generates a transaction and sends it to `sleet`")
	.arg(
            Arg::with_name("peer-ip")
                .short("p")
                .long("peer-ip")
                .value_name("PEER_IP")
                .takes_value(true),
        )
        .get_matches();

    let peer_ip = value_t!(matches.value_of("peer-ip"), SocketAddr)
	.unwrap_or_else(|e| e.exit());

    // Use a key provided by genesis
    // let keypair = hex::decode(..);

    // Select a spendable transaction (ask mempool about known transactions)
    
    // let tx = select_spendable();

    // Construct a transaction, spending some random low amount

    // let transfer_tx = TransferTx::new(&keypair, tx_hash, destination, change, value);
    // let transaction = Transaction::TransferTx(transfer_tx);

    // Send to `sleet`

    // client::oneshot(peer_ip, Request::ReceiveTx(sleet::ReceiveTx {
    // 	tx: transaction,
    // })).await?;

    Ok(())
}
