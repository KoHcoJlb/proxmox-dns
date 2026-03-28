use std::net::Ipv4Addr;

use anyhow::{anyhow, bail, Context, Result};
use macaddr::MacAddr6;
use mikrotik_rs::protocol::command::CommandBuilder;
use mikrotik_rs::protocol::CommandResponse;
use mikrotik_rs::MikrotikDevice;

#[derive(Debug)]
pub struct Lease {
    pub address: Ipv4Addr,
    pub active_mac_address: Option<MacAddr6>,
}

pub struct Client {
    device: MikrotikDevice,
}

impl Client {
    pub async fn connect(addr: &str, username: &str, password: &str) -> Result<Self> {
        Ok(Self {
            device: MikrotikDevice::connect(addr, username, Some(password)).await?,
        })
    }

    pub async fn leases(&self) -> Result<Vec<Lease>> {
        let mut res = self
            .device
            .send_command(
                CommandBuilder::new()
                    .command("/ip/dhcp-server/lease/print")
                    .build(),
            )
            .await
            .context("execute command")?;

        let mut leases = vec![];
        while let Some(msg) = res.recv().await {
            match msg.context("routeros msg")? {
                CommandResponse::Reply(res) => leases.push(Lease {
                    address: res
                        .attributes
                        .get("address")
                        .map(|i| i.as_ref())
                        .flatten()
                        .ok_or(anyhow!("no address: {res:?}"))?
                        .parse()
                        .context(format!("parse address: {res:?}"))?,
                    active_mac_address: res
                        .attributes
                        .get("active-mac-address")
                        .map(|i| i.as_ref())
                        .flatten()
                        .map(|s| {
                            s.parse()
                                .context(format!("parse active-mac-address: {res:?}"))
                        })
                        .transpose()?,
                }),
                CommandResponse::Done(_) => {}
                res => bail!("unexpected response: {:?}", res),
            }
        }

        Ok(leases)
    }
}
