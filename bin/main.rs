use zfx_subzero::Result;
use zfx_subzero::view::{self, View};
use zfx_subzero::server::{Router, Server};
use zfx_subzero::ice::{self, Reservoir, Ice};
use zfx_subzero::chain::alpha::Alpha;
use zfx_subzero::util;

use tracing_subscriber;
use tracing::info;

use actix::{Arbiter, Actor};
use actix_rt::System;

use clap::{value_t, values_t, App, Arg};

use std::net::SocketAddr;
use std::path::Path;

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
        .get_matches();

    let listener_ip = value_t!(matches.value_of("listener-ip"), SocketAddr)
	.unwrap_or_else(|e| e.exit());
    let node_id = util::id_from_ip(&listener_ip);

    let bootstrap_ips = values_t!(matches.values_of("bootstrap-ip"), SocketAddr)
     	.unwrap_or_else(|e| e.exit());

    let system = System::new();

    let execution = async move {
	// Initialise a view with the bootstrap ips and start its actor
	let mut view = View::new(listener_ip);
	view.init(bootstrap_ips);
	let view_addr = view.start();

	// Create the `ice` actor
	let reservoir = Reservoir::new();
	let ice = Ice::new(node_id, listener_ip, reservoir);
	let ice_addr = ice.start();
    
	// Create the `alpha` actor
	let node_id_str = hex::encode(node_id.as_bytes());
	let db_path = vec!["/tmp/", &node_id_str, "/alpha.sled"].concat();
	let alpha = Alpha::create(Path::new(&db_path), ice_addr.clone()).unwrap();
	let alpha_addr = alpha.start();

	// Bootstrap the view
	let view_addr_clone = view_addr.clone();
	let ice_addr_clone = ice_addr.clone();
	let alpha_addr_clone = alpha_addr.clone();

	let bootstrap_execution = async move {
	    view::bootstrap(node_id, view_addr_clone.clone(), ice_addr_clone.clone()).await;
	    let view_addr_clone = view_addr_clone.clone();
	    let ice_addr_clone = ice_addr_clone.clone();
	    let ice_execution = async move {
		// Setup `ice` consensus for establishing the liveness of peers
		ice::run(node_id, ice_addr_clone, view_addr_clone, alpha_addr_clone).await;
	    };
	    let arbiter = Arbiter::new();
	    arbiter.spawn(ice_execution);
	};

	let listener_execution = async move {
	    // Setup the router
	    let router = Router::new(view_addr, ice_addr, alpha_addr);
	    let router_addr = router.start();
	    // Setup the server
	    let server = Server::new(listener_ip, router_addr);
	    // Listen for incoming connections
	    server.listen().await.unwrap()
	};

	let arbiter = Arbiter::new();
	arbiter.spawn(bootstrap_execution);
	arbiter.spawn(listener_execution);
    };
    
    let arbiter = Arbiter::new();
    arbiter.spawn(execution);

    system.run().unwrap();

    Ok(())
}
