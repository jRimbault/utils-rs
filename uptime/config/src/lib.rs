use serde::Deserialize;
use std::{collections::HashMap, net::IpAddr, num::NonZeroU16, time::Duration};

#[derive(Debug, Deserialize, Clone)]
#[serde(untagged)]
pub enum Host {
    Address(IpAddr),
    Hostname(String),
}

#[serde_with::serde_as]
#[derive(Debug, Deserialize, Clone)]
struct ServerSerde {
    host: Host,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    ports: Vec<NonZeroU16>,
    #[serde_as(as = "Option<serde_with::DurationMilliSeconds<u64>>")]
    #[serde(default)]
    interval: Option<Duration>,
    #[serde_as(as = "Option<serde_with::DurationSeconds<u64>>")]
    #[serde(default)]
    period: Option<Duration>,
}

#[derive(Debug, Clone)]
pub struct Server {
    pub host: Host,
    pub ports: Vec<u16>,
    pub interval: Option<Duration>,
    pub period: Option<Duration>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ConfigSerde {
    #[serde(flatten)]
    servers: HashMap<String, ServerSerde>,
}

#[derive(Debug, Clone)]
pub struct Config {
    pub servers: HashMap<String, Server>,
}

impl Server {
    fn from_ser(server: &ServerSerde) -> Server {
        let ServerSerde {
            host,
            ports,
            interval,
            period,
        } = server;
        Server {
            host: host.clone(),
            ports: ports.iter().map(|p| p.get()).collect(),
            interval: *interval,
            period: *period,
        }
    }
}

impl ConfigSerde {
    pub fn server(&self, name: &str) -> Option<Server> {
        self.servers.get(name).map(Server::from_ser)
    }
    pub fn usable(self) -> Config {
        let ConfigSerde { servers } = self;
        Config {
            servers: servers
                .into_iter()
                .map(|(name, server)| (name, Server::from_ser(&server)))
                .collect(),
        }
    }
}
