use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use futures::join;
use hickory_server::authority::{Authority, AuthorityObject, Catalog, ZoneType};
use hickory_server::proto::rr::{Name, Record};
use hickory_server::proto::rr::rdata::{A, SOA};
use hickory_server::ServerFuture;
use hickory_server::store::in_memory::InMemoryAuthority;
use serde::Deserialize;
use tokio::{task, time};
use tokio::net::{TcpListener, UdpSocket};
use tracing::{debug, error, info};
use tracing_subscriber::EnvFilter;

use crate::host::Host;

mod proxmox;
mod routeros;
mod host;

const ALL_DOMAIN: &str = "_all";

#[derive(Debug, Deserialize)]
struct PveConfig {
    url: String,
    username: String,
    tokenid: String,
    node: String,
}

#[derive(Debug, Deserialize)]
struct RosConfig {
    url: String,
    username: String,
    password: String,
}

#[derive(Debug, Deserialize)]
struct Config {
    domain: Name,
    pve: PveConfig,
    ros: RosConfig,
}

fn create_zone(config: &Config) -> InMemoryAuthority {
    let soa = Record::from_rdata(config.domain.clone(), 300, SOA::new(
        config.domain.clone(), config.domain.clone(), 0, 0, 0, 0, 0,
    ));
    let mut zone = InMemoryAuthority::empty(config.domain.clone(), ZoneType::Primary, true);
    zone.upsert_mut(soa.into_record_of_rdata(), 0);
    zone
}

async fn update_loop(config: Config, pve_client: proxmox::Client, ros_client: routeros::Client, zone: Arc<InMemoryAuthority>) {
    async fn run(config: &Config, pve_client: &proxmox::Client, ros_client: &routeros::Client, zone: &InMemoryAuthority) -> Result<()> {
        let (vms, cts, leases) = join!(
            pve_client.virtual_machines(&config.pve.node),
            pve_client.containers(&config.pve.node),
            ros_client.leases()
        );

        let vms = vms.context("fetch vms")?;
        let cts = cts.context("fetch containers")?;
        let leases = leases.context("fetch leases")?;

        let hosts: Vec<Host> = vms.into_iter()
            .filter_map(|vm| ((&vm, leases.as_slice())).try_into()
                .map(Some)
                .unwrap_or_else(|err| {
                    error!(?err, ?vm, "make host from vm");
                    None
                }))
            .chain(cts.into_iter()
                .filter_map(|ct| (&ct, leases.as_slice()).try_into()
                    .map(Some)
                    .unwrap_or_else(|err| {
                        error!(?err, ?ct, "make host from container");
                        None
                    })))
            .collect();

        let records = hosts.into_iter()
            .flat_map(|h| h.ips.into_iter().flat_map(|ip| {
                let zone = Some(zone.origin().into());
                let data = A::from(ip);
                [
                    Record::from_rdata(Name::parse(&h.name, zone.as_ref()).unwrap(), 60, data),
                    Record::from_rdata(Name::parse(ALL_DOMAIN, zone.as_ref()).unwrap(), 60, data)
                ]
            }).collect::<Vec<_>>());

        let mut tmp_zone = create_zone(config);
        for r in records {
            tmp_zone.upsert_mut(r.into_record_of_rdata(), 0);
        }

        let mut records = zone.records_mut().await;
        records.clear();
        records.extend(tmp_zone.records_get_mut().clone().into_iter());

        Ok(())
    }

    loop {
        if let Err(err) = run(&config, &pve_client, &ros_client, &zone).await {
            error!(?err, "update error");
        } else {
            debug!("updated zone");
        }

        time::sleep(Duration::from_secs(30)).await;
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env()
            .unwrap_or(EnvFilter::from_str("proxmox_dns=debug").unwrap()))
        .init();

    let config: Config = config::Config::builder()
        .add_source(config::Environment::with_prefix("PDNS").separator("_"))
        .build()
        .unwrap()
        .try_deserialize().context("load config from env")?;

    info!("Started");

    let zone = Arc::new(create_zone(&config));

    let pve_client = proxmox::Client::new(&config.pve.url, &config.pve.username, &config.pve.tokenid)?;
    let ros_client = routeros::Client::new(&config.ros.url, &config.ros.username, &config.ros.password)?;

    task::spawn(update_loop(config, pve_client, ros_client, zone.clone()));

    let mut catalog = Catalog::new();
    catalog.upsert(zone.origin().clone(), Box::new(zone));

    let mut server = ServerFuture::new(catalog);
    server.register_socket(UdpSocket::bind("0.0.0.0:5354").await?);
    server.register_listener(TcpListener::bind("0.0.0.0:5354").await?,
                             Duration::from_secs(5));
    server.block_until_done().await?;

    Ok(())
}
