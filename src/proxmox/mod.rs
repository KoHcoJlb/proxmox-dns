use reqwest::header::{HeaderMap, AUTHORIZATION};
use reqwest::Url;
use serde::Deserialize;
use thiserror::Error;

pub mod lxc;
mod ser;
pub mod vm;

#[derive(Deserialize)]
struct Data<D> {
    data: D,
}

#[derive(Error, Debug)]
pub enum Error {
    #[error("request error: {0}")]
    Request(#[from] reqwest::Error),
    #[error("deserialization error: {err}: {text}")]
    Serde {
        err: serde_json::Error,
        text: String,
    },
    #[error("parse error")]
    Parse,
}

type Result<T, E = Error> = std::result::Result<T, E>;

fn deserialize<T>(text: String) -> Result<T>
where
    for<'a> T: Deserialize<'a>,
{
    serde_json::from_str(&text).map_err(|err| Error::Serde { err, text })
}

pub struct Client {
    url: Url,
    username: String,
    tokenid: String,

    client: reqwest::Client,
}

impl Client {
    pub fn new(url: &str, username: &str, tokenid: &str) -> anyhow::Result<Self> {
        Ok(Self {
            url: url.try_into()?,
            username: username.into(),
            tokenid: tokenid.into(),
            client: reqwest::Client::builder()
                .danger_accept_invalid_certs(true)
                .build()?,
        })
    }

    fn auth_header(&self) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(
            AUTHORIZATION,
            format!("PVEAPIToken={}={}", self.username, self.tokenid)
                .try_into()
                .unwrap(),
        );
        headers
    }

    fn endpoint<S: AsRef<str>>(&self, path: S) -> Url {
        let mut u = self.url.clone();
        u.set_path(&format!("/api2/json/{}", path.as_ref()));
        u
    }

    async fn get<S: AsRef<str>, T: for<'a> Deserialize<'a>>(&self, path: S) -> Result<T> {
        Ok(deserialize::<Data<T>>(
            self.client
                .get(self.endpoint(path))
                .headers(self.auth_header())
                .send()
                .await?
                .text()
                .await?,
        )?
        .data)
    }
}
