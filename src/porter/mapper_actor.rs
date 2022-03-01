use actix::AsyncContext;
use actix::{Arbiter, Handler};
use std::{net::Ipv4Addr, time::Duration};

use actix::{Actor, Context};
use tracing::{error, trace, warn};

use super::{
    messages::{GetExternalIpMessage, MappingMessage},
    params::RouterConfig,
    Error,
};
use crate::porter::gateway::Gateway;

#[derive(Debug)]
pub struct MapperActor {
    pub config: RouterConfig,
}

impl Actor for MapperActor {
    type Context = Context<Self>;

    fn started(&mut self, _ctx: &mut Context<Self>) {
        trace!("mapper actor started")
    }

    fn stopped(&mut self, _ctx: &mut Context<Self>) {
        trace!("mapper actor stopped")
    }
}

impl Handler<MappingMessage> for MapperActor {
    type Result = std::result::Result<(), Error>;

    fn handle(&mut self, msg: MappingMessage, ctx: &mut Context<Self>) -> Self::Result {
        match msg {
            MappingMessage::AddMapping(add_msg) => {
                let gw = Gateway::new(self.config);
                gw.add_mapping(add_msg)
            }
            MappingMessage::RefreshMapping(refresh_msg) => {
                let conf = self.config.clone();
                let arb = Arbiter::new();

                ctx.run_interval(Duration::from_secs(10), move |_act, _ctx| {
                    let par = refresh_msg.clone();

                    arb.spawn(async move {
                        let gw = Gateway::new(conf);
                        let current_external_ip =
                            gw.get_external_ip().expect("GetExternalIp error!");

                        // TODO: Error log might be enough for now
                        if current_external_ip != par.external_ip {
                            warn!(
                                "External IP has changed! Old: {}, New: {}",
                                par.external_ip, current_external_ip
                            );
                        }

                        match gw.add_mapping(par.add_params) {
                            Ok(()) => trace!("Port lease has been refreshed!"),
                            Err(e) => error!("Port lease refresh failed: {}", e),
                        }
                    });
                });

                Ok(())
            }
        }
    }
}

impl Handler<GetExternalIpMessage> for MapperActor {
    type Result = std::result::Result<Ipv4Addr, Error>;

    fn handle(&mut self, _msg: GetExternalIpMessage, _ctx: &mut Context<Self>) -> Self::Result {
        let gw = Gateway::new(self.config);
        let ext_ip = gw.get_external_ip()?;

        Ok(ext_ip)
    }
}

mod test {
    use std::net::SocketAddrV4;

    use super::*;
    use crate::porter::{
        params::{AddMappingEntry, RefreshMappingEntry},
        Protocol,
    };

    #[actix_rt::test]
    async fn add_port_mapping() {
        let _ = tracing_subscriber::fmt::try_init();

        let mapper_actor = MapperActor { config: RouterConfig::default() }.start();
        let refresh_actor = MapperActor { config: RouterConfig::default() }.start();

        let current_external_ip =
            mapper_actor.send(GetExternalIpMessage::GetExternalIp).await.unwrap();
        assert!(current_external_ip.is_ok());

        let add_params = AddMappingEntry::new(
            SocketAddrV4::new(Ipv4Addr::new(127, 0, 0, 1), 24567),
            24567,
            Protocol::TCP,
            Duration::from_secs(60),
            "zfx_node_add_port_mapping_test".to_string(),
        );

        let add_res =
            mapper_actor.send(MappingMessage::AddMapping(add_params.clone())).await.unwrap();
        assert!(add_res.is_ok());

        let refresh_params = RefreshMappingEntry::new(
            Duration::from_secs(10),
            current_external_ip.unwrap(),
            add_params,
        );

        let refresh_res = refresh_actor
            .send(MappingMessage::RefreshMapping(refresh_params.clone()))
            .await
            .unwrap();
        assert!(refresh_res.is_ok());
    }
}
