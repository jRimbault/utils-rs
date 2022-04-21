use serde::Deserialize;
use std::{
    collections::HashMap,
    net::{IpAddr, SocketAddr},
    time::Duration,
};

#[derive(Debug, Deserialize, Clone)]
#[serde(untagged)]
pub enum Host {
    Address(IpAddr),
    Hostname(String),
}

#[serde_with::serde_as]
#[derive(Debug, Deserialize, Clone)]
pub struct Server {
    pub servers: Vec<SocketAddr>,
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
    pub servers: HashMap<String, Server>,
}
