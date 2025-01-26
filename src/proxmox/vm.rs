use std::collections::BTreeMap;

use futures::future::join_all;
use serde::Deserialize;
use tracing::error;

use super::ser::{Prefix, Spec};
use super::{Client, Result};

#[derive(Debug, Default)]
pub struct VMNet;

impl Prefix for Spec<VMNet> {
    const PREFIX: &'static str = "net";
}

#[derive(Deserialize, Debug)]
pub struct VirtualMachineConfig {
    #[serde(flatten, with = "Spec")]
    pub nets: BTreeMap<u32, Spec<VMNet>>,
}

#[derive(Deserialize, Debug)]
pub struct VirtualMachine {
    pub vmid: u32,
    pub name: String,
    #[serde(skip)]
    pub config: Option<VirtualMachineConfig>,
}

impl Client {
    async fn virtual_machine_config(&self, node: &str, vmid: u32) -> Result<VirtualMachineConfig> {
        self.get(format!("nodes/{node}/qemu/{}/config", vmid)).await
    }

    pub async fn virtual_machines(&self, node: &str) -> Result<Vec<VirtualMachine>> {
        let mut vms: Vec<VirtualMachine> = self.get(format!("nodes/{node}/qemu")).await?;

        let configs = join_all(
            vms.iter()
                .map(|vm| self.virtual_machine_config(node, vm.vmid)),
        )
        .await;
        for (vm, config) in vms.iter_mut().zip(configs) {
            match config {
                Ok(config) => vm.config = Some(config),
                Err(err) => error!(vmid = vm.vmid, ?err, "get vm config"),
            }
        }

        Ok(vms)
    }
}
