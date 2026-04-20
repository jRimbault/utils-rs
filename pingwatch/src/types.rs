//! Domain-level newtypes shared across the crate.

use std::{io, net::IpAddr, sync::Arc};

/// A hostname or IP-address string, validated at the CLI boundary.
#[derive(Clone, Debug)]
pub struct Hostname(Arc<str>);

impl Hostname {
    pub fn as_str(&self) -> &str {
        self.0.as_ref()
    }

    /// Resolves this hostname or IP-address string to its first `IpAddr`.
    ///
    /// Tries a direct parse first (handles bare IP literals without a DNS
    /// round-trip), then falls back to `tokio::net::lookup_host`.
    pub async fn resolve(&self) -> Result<IpAddr, ResolveError> {
        let host = self.0.as_ref();
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
        self.0.fmt(f)
    }
}

impl<'de> serde::Deserialize<'de> for Hostname {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = <String as serde::Deserialize>::deserialize(deserializer)?;
        Ok(Self(Arc::from(value)))
    }
}

impl std::str::FromStr for Hostname {
    type Err = std::convert::Infallible;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Hostname(Arc::from(s)))
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
}
