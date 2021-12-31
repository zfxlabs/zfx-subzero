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
        .about("Runs a simple test oneshot client")
        .arg(
	    Arg::with_name("self-ip")
		.short("s")
		.long("self-ip")
		.value_name("SELF_IP")
		.takes_value(true),
	)
	.arg(
            Arg::with_name("peer-ip")
                .short("p")
                .long("peer-ip")
                .value_name("PEER_IP")
                .takes_value(true),
        )
        .get_matches();

    let self_ip = value_t!(matches.value_of("self-ip"), SocketAddr)
	.unwrap_or_else(|e| e.exit());
    let self_id = id_from_ip(&self_ip);

    let peer_ip = value_t!(matches.value_of("peer-ip"), SocketAddr)
	.unwrap_or_else(|e| e.exit());

    client::oneshot(peer_ip, Request::Version(version::Version {
	id: self_id,
	ip: self_ip,
    })).await;

    Ok(())
}
