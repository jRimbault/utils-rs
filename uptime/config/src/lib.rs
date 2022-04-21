use indexmap::IndexMap;
use serde::Deserialize;
use std::{net::SocketAddr, time::Duration};

#[derive(Debug, Deserialize, Clone)]
#[serde(untagged)]
pub enum Address {
    Direct(SocketAddr),
    Dns(String),
}

#[serde_with::serde_as]
#[derive(Debug, Deserialize, Clone)]
pub struct Server {
    pub servers: Vec<Address>,
    #[serde(default)]
    #[serde_as(as = "Option<serde_with::DurationMilliSeconds<u64>>")]
    pub interval: Option<Duration>,
    #[serde(default)]
    #[serde_as(as = "Option<serde_with::DurationSeconds<u64>>")]
    pub period: Option<Duration>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    #[serde(flatten)]
    pub servers: IndexMap<String, Server>,
}
