//! This is a program used to bootstrap a network, currently being used for testing. Please use the
//! `node` executable unless creating a new network for the first time.

use zfx_subzero::alpha::Alpha;
use zfx_subzero::client::Client;
use zfx_subzero::p2p::id::Id;
use zfx_subzero::p2p::linear_backoff::{LinearBackoff, Start};
use zfx_subzero::p2p::peer_bootstrapper::PeerBootstrapper;
use zfx_subzero::p2p::primary_bootstrapper::PrimaryBootstrapper;
use zfx_subzero::protocol::{Request, Response};
use zfx_subzero::server::{Router, Server};
use zfx_subzero::tls;
use zfx_subzero::Result;

use zfx_subzero::message::Version;
use zfx_subzero::p2p::peer_meta::PeerMetadata;

use ed25519_dalek::Keypair;
use std::collections::HashSet;
use std::net::SocketAddr;
use std::path::Path;
use std::time::Duration;

use actix::{Actor, Arbiter};

use tokio;
use tracing::info;
use tracing_subscriber;

use clap::{value_t, values_t, App, Arg};

fn main() -> Result<()> {
    tracing_subscriber::fmt().compact().with_max_level(tracing::Level::INFO).init();

    // CLI parameters
    let matches = App::new("zfx-subzero")
        .version("0.1")
        .author("zero.fx labs ltd.")
        .about("Generates a transaction and sends it to `sleet`")
        // FIXME `id@ip` here
        .arg(
            Arg::with_name("self-ip")
                .short("a")
                .long("self-ip")
                .value_name("SELF_IP")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("peer")
                .short("b")
                .long("peer")
                .value_name("PEER_ID@PEER_IP")
                .multiple(true),
        )
        .arg(
            Arg::with_name("keypair")
                .short("k")
                .long("keypair")
                .value_name("KEYPAIR_HEX")
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
        .get_matches();

    // The ip of this node.
    let self_ip = value_t!(matches.value_of("self-ip"), String).unwrap_or_else(|e| e.exit());
    // Bootstrap peers to be contacted (many)
    let remote_peers = values_t!(matches.values_of("peer"), String).unwrap_or_else(|e| e.exit());
    // The keypair that owns the `txhash` for spending
    let keypair = value_t!(matches.value_of("keypair"), String).unwrap_or_else(|e| e.exit());
    // Whether to use a TLS or plain TCP upgrader
    let use_tls = matches.is_present("use-tls");

    // The chains the bootstrapping node wishes to bootstrap
    let mut chains = HashSet::new();
    chains.insert(Id::one());

    // Clones trusted remote peer metadata (assumes peer chains are the same)
    let remote_peer_metas = remote_peers
        .iter()
        .map(|p| PeerMetadata::from_id_and_ip(p, chains.clone()).unwrap())
        .collect::<Vec<PeerMetadata>>();

    // The TLS certificate path (if using TLS)
    let cert_path = if use_tls {
        Some(value_t!(matches.value_of("cert-path"), String).unwrap_or_else(|e| e.exit()))
    } else {
        None
    };

    // The TLS private key path (if using TLS)
    let priv_key_path = if use_tls {
        Some(value_t!(matches.value_of("pk-path"), String).unwrap_or_else(|e| e.exit()))
    } else {
        None
    };

    // This nodes `ip` address
    let self_ip = self_ip.parse().unwrap();

    // TCP/TLS setup and create the `node_id` (`Id`)
    let (client_upgrader, server_upgrader, node_id) = if use_tls {
        let (cert, key) = tls::certificate::get_node_cert(
            Path::new(&cert_path.unwrap()),
            Path::new(&priv_key_path.unwrap()),
        )
        .unwrap();
        let upgraders = tls::upgrader::tls_upgraders(&cert, &key);
        (upgraders.client, upgraders.server, Id::new(&cert))
    } else {
        (
            tls::upgrader::TcpUpgrader::new(),
            tls::upgrader::TcpUpgrader::new(),
            Id::from_ip(&self_ip),
        )
    };

    // The self is subscribed to the primary chain `Id::one()`
    let self_peer_meta = PeerMetadata { id: node_id, ip: self_ip, chains: chains.clone() };

    // Reconstruct the provided keypair
    let keypair_bytes = hex::decode(keypair).unwrap();
    let keypair = Keypair::from_bytes(&keypair_bytes).unwrap();
    let encoded = bincode::serialize(&keypair.public).unwrap();
    let pkh = blake3::hash(&encoded).as_bytes().clone();

    let sys = actix::System::new();
    sys.block_on(async move {
        let self_peer_meta = self_peer_meta.clone();
        let send_timeout = Duration::from_millis(1000);

        // Primary chain configuration
        let primary_chain_id = Id::one();
        let primary_chain_id_s = format!("{}", primary_chain_id);
        let bootstrap_peer_lim = 2;
        let node_id_str = hex::encode(node_id.as_bytes());
        let chain_db_path =
            vec!["/tmp/", &node_id_str, "/", &primary_chain_id_s, "/alpha.sled"].concat();
        // Primary chain initialisation (alpha chain protocol)
        let alpha_address = Alpha::new(primary_chain_id.clone(), chain_db_path).start();

        // Setup the initial router (actors should add themselves to the router as they spawn)
        let router = Router::new(self_peer_meta.clone(), alpha_address.clone());
        let router_address = router.start();
        let router_address_clone = router_address.clone();

        let client_execution = async move {
            // Primary bootstrapper init
            info!("initialising primary bootstrapper");
            let mut primary_bootstrapper = PrimaryBootstrapper::new(
                client_upgrader.clone(),
                self_peer_meta.clone(),
                primary_chain_id,
                bootstrap_peer_lim,
                router_address.clone(),
                alpha_address.clone(),
            );
            let primary_bootstrapper_address = primary_bootstrapper.start();
            let primary_bootstrapper_recipient = primary_bootstrapper_address.recipient().clone();

            let mut trusted_peers = remote_peer_metas.clone();
            trusted_peers.push(self_peer_meta.clone());
            let trusted_peer_discovery_limit = 3;
            let iteration_limit = 5;
            let peer_bootstrapper_address = PeerBootstrapper::new(
                client_upgrader,
                self_peer_meta,
                trusted_peers,
                trusted_peer_discovery_limit,
                iteration_limit,
                primary_bootstrapper_recipient,
                send_timeout,
            )
            .start();
            info!("initial backoff delay = 1s");
            let init_backoff_delay = Duration::from_millis(1000);
            let backoff =
                LinearBackoff::new(peer_bootstrapper_address.recipient(), init_backoff_delay)
                    .start();
            let () = backoff.do_send(Start);
        };
        let server_execution = async move {
            let server = Server::new(self_ip, router_address_clone, server_upgrader);
            server.listen().await.unwrap()
        };
        let arbiter = Arbiter::new();
        arbiter.spawn(client_execution);
        arbiter.spawn(server_execution);

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
        info!(target: "zfx_subzero", "Got {}, stopping...", sig);

        actix::System::current().stop();
    });
    sys.run().unwrap();

    Ok(())
}
