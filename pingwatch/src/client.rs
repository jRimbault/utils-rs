use std::net::IpAddr;

/// Shared ICMP clients, one per protocol version.
///
/// Created once in [`crate::run`] and cloned into each worker task.
/// `surge_ping::Client` is `Arc`-backed internally, so cloning is cheap
/// and all workers share the same underlying socket per protocol.
#[derive(Clone)]
pub struct PingClients {
    inner: std::sync::Arc<Inner>,
}

#[derive(Clone)]
struct Inner {
    v4: surge_ping::Client,
    v6: surge_ping::Client,
}

impl PingClients {
    pub fn new() -> std::io::Result<Self> {
        Ok(Self {
            inner: std::sync::Arc::new(Inner {
                v4: surge_ping::Client::new(
                    &surge_ping::Config::builder()
                        .kind(surge_ping::ICMP::V4)
                        .build(),
                )?,
                v6: surge_ping::Client::new(
                    &surge_ping::Config::builder()
                        .kind(surge_ping::ICMP::V6)
                        .build(),
                )?,
            }),
        })
    }

    pub(crate) fn for_addr(&self, addr: IpAddr) -> &surge_ping::Client {
        match addr {
            IpAddr::V4(_) => &self.inner.v4,
            IpAddr::V6(_) => &self.inner.v6,
        }
    }
}
