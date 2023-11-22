use std::collections::BTreeMap;

use futures::future::join_all;
use serde::Deserialize;
use serde_with::{DisplayFromStr, serde_as};
use tracing::error;

use super::{Client, Result};
use super::ser::{Prefix, Spec};

#[derive(Debug, Default)]
pub struct LXCNet;

impl Prefix for Spec<LXCNet> {
    const PREFIX: &'static str = "net";
}

#[derive(Deserialize, Debug)]
pub struct ContainerConfig {
    #[serde(flatten, with = "Spec")]
    pub nets: BTreeMap<u32, Spec<LXCNet>>,
}

#[serde_as]
#[derive(Deserialize, Debug)]
pub struct Container {
    #[serde_as(as = "DisplayFromStr")]
    pub vmid: u32,
    pub name: String,
    #[serde(skip)]
    pub config: Option<ContainerConfig>,
}

impl Client {
    async fn container_config(&self, node: &str, vmid: u32) -> Result<ContainerConfig> {
        self.get(format!("nodes/{node}/lxc/{vmid}/config")).await
    }

    pub async fn containers(&self, node: &str) -> Result<Vec<Container>> {
        let mut cts: Vec<Container> = self.get(format!("nodes/{node}/lxc")).await?;

        let configs = join_all(cts.iter()
            .map(|vm| self.container_config(node, vm.vmid)))
            .await;
        for (vm, config) in cts.iter_mut().zip(configs) {
            match config {
                Ok(config) => vm.config = Some(config),
                Err(err) => error!(vmid = vm.vmid, ?err, "get container config")
            }
        }

        Ok(cts)
    }
}
