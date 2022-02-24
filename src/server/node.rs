use std::io::{BufReader, Read, Write};
use std::net::{SocketAddr, ToSocketAddrs};
use std::path::Path;

use crate::alpha::Alpha;
use actix::{Actor, Arbiter};
use ed25519_dalek::Keypair;
use rand::rngs::OsRng;
use tracing::info;
use tracing_subscriber;

use crate::client::Client;
use crate::hail::Hail;
use crate::ice::{self, Ice, Reservoir};
use crate::server::{Router, Server};
use crate::sleet::Sleet;
use crate::tls;
use crate::view::{self, View};
use crate::zfx_id::Id;
use crate::Result;

pub fn run(ip: String, bootstrap_ips: Vec<String>, keypair: Option<String>, use_tls: bool) -> Result<()> {
    let listener_ip: SocketAddr = ip.parse().unwrap();
    let node_id = Id::from_ip(&listener_ip);
    let node_id_str = hex::encode(node_id.as_bytes());

    match keypair {
        Some(keypair_hex) => {
            let dir_path = vec!["/tmp/", &node_id_str].concat();
            let file_path = vec!["/tmp/", &node_id_str, "/", &node_id_str, ".keypair"].concat();
            std::fs::create_dir_all(&dir_path)
                .expect(&format!("Couldn't create directory: {}", dir_path));
            let mut file = std::fs::File::create(file_path).unwrap();
            file.write_all(keypair_hex.as_bytes()).unwrap();
            let keypair_bytes = hex::decode(keypair_hex).unwrap();
            Keypair::from_bytes(&keypair_bytes).unwrap()
        }
        None => panic!("Keypair is mandatory"),
    };

    let converted_bootstrap_ips =
        bootstrap_ips.iter().map(|ip| ip.parse().unwrap()).collect::<Vec<SocketAddr>>();

    // This is temporary until we have TLS setup
    let upgraders = if use_tls {
        tls::upgrader::tcp_upgraders()
    } else {
        tls::upgrader::tcp_upgraders()
    };
    let execution = async move {
        // Create the 'client' actor
        let client = Client::new(upgraders.client.clone());
        let client_addr = client.start();

        // Initialise a view with the bootstrap ips and start its actor
        let mut view = View::new(client_addr.clone().recipient(), listener_ip);
        view.init(converted_bootstrap_ips);
        let view_addr = view.start();

        // Create the `ice` actor
        let reservoir = Reservoir::new();
        let ice = Ice::new(client_addr.clone().recipient(), node_id, listener_ip, reservoir);
        let ice_addr = ice.start();

        // Create the `hail` actor
        let hail = Hail::new(client_addr.clone().recipient(), node_id);
        let hail_addr = hail.start();

        // Create the `sleet` actor
        // FIXME: Sleet has to be initialised with the genesis utxo ids.
        let sleet = Sleet::new(
            client_addr.clone().recipient(),
            hail_addr.clone().recipient(),
            node_id,
            listener_ip,
        );
        let sleet_addr = sleet.start();

        // Create the `alpha` actor
        let db_path = vec!["/tmp/", &node_id_str, "/alpha.sled"].concat();
        let alpha = Alpha::create(
            client_addr.clone().recipient(),
            node_id,
            Path::new(&db_path),
            ice_addr.clone(),
            sleet_addr.clone(),
            hail_addr.clone(),
        )
        .unwrap();
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
            let router = Router::new(view_addr, ice_addr, alpha_addr, sleet_addr, hail_addr);
            let router_addr = router.start();
            // Setup the server
            let server = Server::new(listener_ip, router_addr, upgraders.server.clone());
            // Listen for incoming connections
            server.listen().await.unwrap()
        };

        let arbiter = Arbiter::new();
        arbiter.spawn(bootstrap_execution);
        arbiter.spawn(listener_execution);
    };

    let arbiter = Arbiter::new();
    arbiter.spawn(execution);

    Ok(())
}

fn read_or_generate_keypair(node_id: String) -> Result<Keypair> {
    let tmp_dir = vec!["/tmp/", &node_id].concat();
    std::fs::create_dir_all(&tmp_dir).expect(&format!("Couldn't create directory: {}", tmp_dir));
    let keypair_path = vec![&tmp_dir[..], "/", &node_id, ".keypair"].concat();
    match std::fs::File::open(keypair_path.clone()) {
        Ok(file) => {
            let mut buf_reader = BufReader::new(file);
            let mut contents = String::new();
            buf_reader.read_to_string(&mut contents)?;
            info!("keypair => {:?}", contents.clone());
            let keypair_bytes = hex::decode(contents).unwrap();
            let keypair = Keypair::from_bytes(&keypair_bytes)?;
            Ok(keypair)
        }
        Err(_) => {
            let dir_path = vec!["/tmp/", &node_id].concat();
            let mut csprng = OsRng {};
            let keypair = Keypair::generate(&mut csprng);
            let keypair_string = hex::encode(keypair.to_bytes());
            info!("keypair => {:?}", keypair_string.clone());
            std::fs::create_dir_all(dir_path).unwrap();
            let mut file = std::fs::File::create(keypair_path)?;
            file.write_all(keypair_string.as_bytes())?;
            Ok(keypair)
        }
    }
}
