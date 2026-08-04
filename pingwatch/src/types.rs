//! Domain-level newtypes shared across the crate.

use std::{io, net::IpAddr, sync::Arc};

/// A ping target: a bare hostname/IP-address string, or an IP address paired
/// with a user-assigned display name (for addresses with no DNS entry worth
/// showing).
#[derive(Clone, Debug)]
pub enum Hostname {
    /// A domain name or IP-address literal, used as-is for both display and
    /// resolution.
    Plain(Arc<str>),
    /// A pre-resolved IP address shown under a custom display name instead
    /// of its literal address.
    Named { name: Arc<str>, addr: IpAddr },
}

impl Hostname {
    /// The label to show in the UI: the plain string, or the custom name.
    pub fn as_str(&self) -> &str {
        match self {
            Self::Plain(s) => s,
            Self::Named { name, .. } => name,
        }
    }

    /// Resolves this target to its `IpAddr`.
    ///
    /// A [`Self::Named`] address is already known and returned directly. A
    /// [`Self::Plain`] value tries a direct parse first (handles bare IP
    /// literals without a DNS round-trip), then falls back to
    /// `tokio::net::lookup_host`.
    pub async fn resolve(&self) -> Result<IpAddr, ResolveError> {
        let host = match self {
            Self::Named { addr, .. } => return Ok(*addr),
            Self::Plain(s) => s.as_ref(),
        };
        if let Ok(ip) = host.parse::<IpAddr>() {
            return Ok(ip);
        }
        let mut addrs = tokio::net::lookup_host(format!("{host}:0"))
            .await
            .map_err(|e| ResolveError::DnsLookup(e.kind()))?;
        addrs
            .next()
            .map(|sa| sa.ip())
            .ok_or(ResolveError::NoAddresses)
    }
}

/// Compact hostname-resolution failure carried across task boundaries.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResolveError {
    DnsLookup(io::ErrorKind),
    NoAddresses,
}

impl std::fmt::Display for ResolveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DnsLookup(kind) => write!(f, "dns lookup failed: {}", io::Error::from(*kind)),
            Self::NoAddresses => f.write_str("no addresses found"),
        }
    }
}

impl std::error::Error for ResolveError {}

impl std::fmt::Display for Hostname {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Config-file shape for a single `hosts` entry: either a bare string
/// (hostname or IP literal) or a table naming an IP address, e.g.
/// `{ name = "router", ip = "192.168.1.1" }`.
#[derive(serde::Deserialize)]
#[serde(untagged)]
enum HostnameConfig {
    Plain(String),
    Named { name: String, ip: String },
}

impl<'de> serde::Deserialize<'de> for Hostname {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        match HostnameConfig::deserialize(deserializer)? {
            HostnameConfig::Plain(value) => Ok(Self::Plain(Arc::from(value))),
            HostnameConfig::Named { name, ip } => {
                let addr = ip
                    .parse::<IpAddr>()
                    .map_err(|e| serde::de::Error::custom(format!("invalid `ip` {ip:?}: {e}")))?;
                Ok(Self::Named {
                    name: Arc::from(name),
                    addr,
                })
            }
        }
    }
}

impl std::str::FromStr for Hostname {
    type Err = std::convert::Infallible;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Hostname::Plain(Arc::from(s)))
    }
}

/// Index of a host's slot in the current run's host list.
///
/// Constructed once in `lib::run` from the enumeration position; all
/// subsequent indexing into `bars` and `hosts` slices goes through this type
/// to prevent confusing it with an unrelated `usize`.
#[derive(Clone, Copy, Debug)]
pub struct HostIdx(usize);

impl HostIdx {
    pub fn new(i: usize) -> Self {
        Self(i)
    }

    pub fn as_usize(self) -> usize {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cloned_hostnames_share_backing_storage() {
        let host = "example.com".parse::<Hostname>().unwrap();
        let clone = host.clone();

        assert!(std::ptr::addr_eq(
            host.as_str().as_ptr(),
            clone.as_str().as_ptr()
        ));
    }

    #[tokio::test]
    async fn named_host_resolves_to_its_fixed_addr_and_displays_its_name() {
        let addr: IpAddr = "192.168.1.1".parse().unwrap();
        let host = Hostname::Named {
            name: Arc::from("router"),
            addr,
        };

        assert_eq!(host.as_str(), "router");
        assert_eq!(host.resolve().await.unwrap(), addr);
    }

    #[test]
    fn named_host_deserializes_from_toml_table() {
        let host: Hostname =
            toml_edit::de::from_str("h = { name = \"router\", ip = \"192.168.1.1\" }")
                .map(|v: std::collections::HashMap<String, Hostname>| {
                    v.into_iter().next().unwrap().1
                })
                .unwrap();

        assert_eq!(host.as_str(), "router");
    }

    #[test]
    fn named_host_rejects_invalid_ip() {
        let result: Result<std::collections::HashMap<String, Hostname>, _> =
            toml_edit::de::from_str("h = { name = \"router\", ip = \"not-an-ip\" }");
        assert!(result.is_err());
    }
}
