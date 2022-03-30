use std::io::{BufReader, Read, Write};
use std::net::SocketAddr;
use std::path::Path;

use crate::alpha::Alpha;
use crate::client::Client;
use crate::hail::Hail;
use crate::ice::dissemination::DisseminationComponent;
use crate::ice::{self, Ice, Reservoir};
use crate::server::{Router, Server, Settings};
use crate::sleet::Sleet;
use crate::tls;
use crate::util;
use crate::view::{self, View};
use crate::zfx_id::Id;
use crate::Result;
use actix::{Actor, Arbiter};
use ed25519_dalek::Keypair;
use rand::rngs::OsRng;
use tracing::info;

pub fn run(settings: Settings, home_dir: &Path) -> Result<()> {
    let listener_ip: SocketAddr = settings.listener_ip.parse().unwrap();
    let converted_bootstrap_peers = settings
        .bootstrap_peers
        .iter()
        .map(|p| util::parse_id_and_ip(p).unwrap())
        .collect::<Vec<(Id, SocketAddr)>>();

    // This is temporary until we have TLS setup
    let (node_id, upgraders) = if settings.use_tls {
        let (cert, key) = tls::certificate::get_node_cert(
            &home_dir.join(Path::new(&settings.certificate_file.unwrap())),
            &home_dir.join(Path::new(&settings.private_key_file.unwrap())),
        )
        .unwrap();
        let upgraders = tls::upgrader::tls_upgraders(&cert, &key);
        (Id::new(&cert), upgraders)
        // FIXME, until we change alpha and genesis
        // (Id::from_ip(&listener_ip), upgraders)
    } else {
        // FIXME, until we change alpha and genesis
        match settings.id {
            None => (Id::from_ip(&listener_ip), tls::upgrader::tcp_upgraders()),
            Some(id) => (id, tls::upgrader::tcp_upgraders()),
        }
    };
    let node_id_str = hex::encode(node_id.as_bytes());

    info!("Node {} is starting", node_id);

    let dir_path = &home_dir.join(Path::new(&node_id_str));
    let file_path = &dir_path.join(node_id_str.to_owned() + ".keypair");
    std::fs::create_dir_all(&dir_path)
        .expect(&format!("Couldn't create directory: {}", dir_path.to_str().unwrap()));
    let mut file = std::fs::File::create(file_path).unwrap();
    file.write_all(settings.keypair.as_bytes()).unwrap();
    let keypair_bytes = hex::decode(settings.keypair).unwrap();
    Keypair::from_bytes(&keypair_bytes).unwrap();

    let db_path = home_dir
        .join(Path::new(&node_id_str))
        .join("alpha.sled")
        .into_os_string()
        .into_string()
        .unwrap();

    let execution = async move {
        // Create the 'client' actor
        let client = Client::new(upgraders.client.clone());
        let client_addr = client.start();

        // Initialise a view with the bootstrap ips and start its actor
        let mut view = View::new(client_addr.clone().recipient(), listener_ip, node_id);
        view.init(converted_bootstrap_peers);
        let view_addr = view.start();

        // Create Dissemination Component
        let dc = DisseminationComponent::new();
        let dc_addr = dc.start();

        // Create the `ice` actor
        let reservoir = Reservoir::new();
        let ice = Ice::new(
            client_addr.clone().recipient(),
            node_id,
            listener_ip,
            reservoir,
            dc_addr.clone().recipient(),
        );
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
            view::bootstrap(view_addr_clone.clone(), ice_addr_clone.clone()).await;
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

#[allow(unused)] // TODO check if we need this after config is done
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
