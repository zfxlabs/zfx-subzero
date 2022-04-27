use tracing::info;
use tracing::trace;
use tracing_subscriber;

use clap::{value_t, values_t, App, Arg};

use std::path::Path;
use std::str::FromStr;
use zfx_subzero::server::node;
use zfx_subzero::server::settings::Settings;
use zfx_subzero::zfx_id;
use zfx_subzero::Result;

const DEFAULT_HOME_DIR: &str = "src/server/settings";

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
        .arg(Arg::with_name("home").short("h").long("home").takes_value(true).required(false))
        // FIXME this is a temporary workaround for tcp nodes
        .arg(Arg::with_name("node-id").long("id").value_name("NODE-ID").takes_value(true))
        .get_matches();

    let home_dir = matches.value_of("home").unwrap_or(DEFAULT_HOME_DIR);
    let mut settings = Settings::new(Path::new(home_dir)).expect("failed to load configuration.");

    if let Some(ip) = matches.value_of("listener-ip") {
        trace!("CLI arg for listener-ip provided: {}", ip);
        settings.listener_ip = ip.to_owned();
    }

    if let Some(peers) = matches.values_of("bootstrap-peer") {
        trace!("CLI arg for bootstrap-peer provided: {:?}", peers);
        settings.bootstrap_peers =
            values_t!(matches.values_of("bootstrap-peer"), String).unwrap_or_else(|e| e.exit());
    }

    if let Some(kp) = matches.value_of("keypair") {
        trace!("CLI arg for keypair provided: {}", kp);
        settings.keypair = kp.to_owned();
    }

    if matches.is_present("use-tls") {
        trace!("CLI arg for use-tls provided");
        settings.use_tls = true;

        settings.certificate_file =
            Some(value_t!(matches.value_of("cert-path"), String).unwrap_or_else(|e| e.exit()));
        settings.private_key_file =
            Some(value_t!(matches.value_of("pk-path"), String).unwrap_or_else(|e| e.exit()));
    };

    if let Some(node_str) = matches.value_of("node-id") {
        settings.id = Some(zfx_id::Id::from_str(node_str).unwrap())
    };
    let sys = actix::System::new();
    sys.block_on(async move {
        node::run(settings, Path::new(home_dir)).unwrap();

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
