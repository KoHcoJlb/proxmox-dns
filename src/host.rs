use std::net::Ipv4Addr;

use anyhow::anyhow;
use tracing::error;

use crate::proxmox::lxc::Container;
use crate::proxmox::vm::VirtualMachine;
use crate::routeros::Lease;

#[derive(Debug)]
pub struct Host {
    pub name: String,
    pub ips: Vec<Ipv4Addr>,
}

impl TryFrom<(&VirtualMachine, &[Lease])> for Host {
    type Error = anyhow::Error;

    fn try_from((vm, leases): (&VirtualMachine, &[Lease])) -> Result<Self, Self::Error> {
        let config = vm.config.as_ref().ok_or(anyhow!("missing config"))?;
        let ips = config.nets
            .values()
            .filter_map(|net| {
                match net.extract_mac() {
                    Ok(mac) => Some(mac),
                    Err(err) => {
                        error!(?err, vmid = vm.vmid, "extract mac failed");
                        None
                    }
                }
            })
            .filter_map(|mac| leases.iter().find(|l| l.active_mac_address == Some(mac)))
            .map(|l| l.address)
            .collect();
        Ok(Self {
            name: vm.name.clone(),
            ips,
        })
    }
}

impl TryFrom<(&Container, &[Lease])> for Host {
    type Error = anyhow::Error;

    fn try_from((ct, leases): (&Container, &[Lease])) -> Result<Self, Self::Error> {
        let config = ct.config.as_ref().ok_or(anyhow!("missing config"))?;
        let ips = config.nets
            .values()
            .filter_map(|net| {
                match net.extract_mac() {
                    Ok(mac) => Some(mac),
                    Err(err) => {
                        error!(?err, vmid = ct.vmid, "extract mac failed");
                        None
                    }
                }
            })
            .filter_map(|mac| leases.iter().find(|l| l.active_mac_address == Some(mac)))
            .map(|l| l.address)
            .collect();
        Ok(Self {
            name: ct.name.clone(),
            ips,
        })
    }
}
