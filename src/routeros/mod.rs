use std::net::Ipv4Addr;
use macaddr::MacAddr6;
use reqwest::Url;
use serde::Deserialize;
use serde_with::{DisplayFromStr, serde_as};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("request error: {0}")]
    Request(#[from] reqwest::Error),
    #[error("deserialization error: {err}: {text}")]
    Serde {
        err: serde_json::Error,
        text: String,
    },
}

type Result<T, E = Error> = std::result::Result<T, E>;

#[serde_as]
#[derive(Deserialize, Debug)]
#[serde(rename_all = "kebab-case")]
pub struct Lease {
    #[serde_as(as = "DisplayFromStr")]
    pub address: Ipv4Addr,
    #[serde_as(as = "DisplayFromStr")]
    pub mac_address: MacAddr6,
}

pub struct Client {
    url: Url,
    username: String,
    password: String,

    client: reqwest::Client,
}

impl Client {
    pub fn new(url: &str, username: &str, password: &str) -> anyhow::Result<Self> {
        Ok(Self {
            url: url.try_into()?,
            username: username.into(),
            password: password.into(),
            client: reqwest::Client::new(),
        })
    }

    pub async fn leases(&self) -> Result<Vec<Lease>> {
        self.get("ip/dhcp-server/lease").await
    }

    async fn get<T: for<'a> Deserialize<'a>>(&self, path: &str) -> Result<T> {
        let mut u = self.url.clone();
        u.set_path(&format!("/rest/{path}"));

        let text = self.client.get(u)
            .basic_auth(&self.username, Some(&self.password))
            .send().await?
            .text().await?;

        Ok(serde_json::from_str(&text).map_err(|err| Error::Serde {
            err,
            text,
        })?)
    }
}
