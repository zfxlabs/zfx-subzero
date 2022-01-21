use tracing::info;
use tracing_subscriber;

use clap::{value_t, values_t, App, Arg};

use zfx_subzero::server::node;
use zfx_subzero::Result;

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_level(false)
        .with_target(false)
        .without_time()
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
            Arg::with_name("bootstrap-ip")
                .short("b")
                .long("bootstrap-ip")
                .value_name("BOOTSTRAP_IP")
                .multiple(true),
        )
        .arg(
            Arg::with_name("keypair")
                .short("kp")
                .long("keypair")
                .value_name("KEYPAIR")
                .takes_value(true)
                .required(false),
        )
        .get_matches();

    let listener_ip =
        value_t!(matches.value_of("listener-ip"), String).unwrap_or_else(|e| e.exit());
    let bootstrap_ips =
        values_t!(matches.values_of("bootstrap-ip"), String).unwrap_or_else(|e| e.exit());
    let keypair = match matches.value_of("keypair") {
        Some(keypair_hex) => Some(String::from(keypair_hex)),
        _ => None,
    };

    let sys = actix::System::new();
    sys.block_on(async move {
        node::run(listener_ip, bootstrap_ips, keypair).unwrap();

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
