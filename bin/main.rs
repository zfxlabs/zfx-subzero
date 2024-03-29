use tracing::info;
use tracing_subscriber;

use clap::{value_t, values_t, App, Arg};

use zfx_subzero::server::node;
use zfx_subzero::zfx_id;
use zfx_subzero::Result;

use std::str::FromStr;

/// An entrypoint for starting up a [node](zfx_subzero::server::node::run).
/// When running from a terminal, accepts the following list of parameters:
/// * `--listener-ip` or `-a` - IP address and port of the node (ex. 127.0.0.1:1234).
/// * `--bootstrap-peer` or `-b` - one or more addresses of running nodes of the network for bootstrapping
/// in format <node_id>@<node_ip_address> (ex. 19Y53ymnBw4LWUpiAMUzPYmYqZmukRhNHm3VyAhzMqckRcuvkf@127.0.0.1:1234).
/// * `--keypair` or `-k` - a hex keypair for the node in String format.
/// * `--use-tls` or `-t` (optional) - indicates whether to use TLS connection.
/// If true, then `cert_path` and `pk_path` are mandatory parameters.
/// If false, then plain TCP connection is used.
/// * `--cert-path` or `-c` (optional) - path to a certificate used in TLS connection. Mandatory parameter if `use_tls` flag is true.
/// A sample of certificate can be found in `./deployment/test-certs/*.crt`.
/// * `--priv-key-path` or `-p` (optional) - path to a private key for the node. Mandatory parameter if `use_tls` flag is true.
/// A sample of private key can be found in `./deployment/test-certs/*.key`
/// * `--id` - Id of a node in a hex String format (ex. 19Y53ymnBw4LWUpiAMUzPYmYqZmukRhNHm3VyAhzMqckRcuvkf).
fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_level(true)
        .with_target(true)
        .compact()
        .with_max_level(tracing::Level::INFO)
        .init();

    let matches = App::new("zfx-subzero")
        .version("0.1")
        .author("zero.fx labs ltd.")
        .about("Runs a zero.fx node")
        .arg(
            Arg::with_name("listener-ip")
                .short("a")
                .long("listener-ip")
                .value_name("LISTENER_IP")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("bootstrap-peer")
                .short("b")
                .long("bootstrap-peer")
                .value_name("BOOTSTRAP_PEER")
                .multiple(true),
        )
        .arg(
            Arg::with_name("keypair")
                .short("k")
                .long("keypair")
                .value_name("KEYPAIR")
                .takes_value(true)
                .required(false),
        )
        .arg(
            Arg::with_name("use-tls").short("t").long("use-tls").required(false).takes_value(false),
        )
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
        // FIXME this is a temporary workaround for tcp nodes
        .arg(Arg::with_name("node-id").long("id").value_name("NODE-ID").takes_value(true))
        .get_matches();

    let listener_ip =
        value_t!(matches.value_of("listener-ip"), String).unwrap_or_else(|e| e.exit());
    let bootstrap_peers =
        values_t!(matches.values_of("bootstrap-peer"), String).unwrap_or_else(|e| e.exit());
    let keypair = match matches.value_of("keypair") {
        Some(keypair_hex) => Some(String::from(keypair_hex)),
        _ => None,
    };
    let use_tls = matches.is_present("use-tls");
    let cert_path = if use_tls {
        Some(value_t!(matches.value_of("cert-path"), String).unwrap_or_else(|e| e.exit()))
    } else {
        None
    };
    let priv_key_path = if use_tls {
        Some(value_t!(matches.value_of("pk-path"), String).unwrap_or_else(|e| e.exit()))
    } else {
        None
    };

    let node_id = match matches.value_of("node-id") {
        Some(node_str) => Some(zfx_id::Id::from_str(node_str).unwrap()),
        _ => None,
    };
    let sys = actix::System::new();
    sys.block_on(async move {
        node::run(
            listener_ip,
            bootstrap_peers,
            keypair,
            use_tls,
            cert_path,
            priv_key_path,
            node_id,
        )
        .unwrap();

        let sig = if cfg!(unix) {
            use futures::future::FutureExt;
            use tokio::signal::unix::{signal, SignalKind};

            let mut sigint = signal(SignalKind::interrupt()).unwrap();
            let mut sigterm = signal(SignalKind::terminate()).unwrap();

            futures::select! {
                _ = sigint.recv().fuse() => "SIGINT",
                _ = sigterm.recv().fuse() => "SIGTERM"
            }
        } else {
            tokio::signal::ctrl_c().await.unwrap();
            "Ctrl+C"
        };
        info!(target: "sub-zero", "Got {}, stopping...", sig);

        actix::System::current().stop();
    });
    sys.run().unwrap();

    Ok(())
}
