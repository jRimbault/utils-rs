use std::{io, net::IpAddr, time::Duration};

use crate::types;

/// Compact ping failure carried from workers to the printer without allocating.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PingFailure {
    IncorrectBufferSize,
    MalformedPacket,
    Io(io::ErrorKind),
    Timeout {
        seq: u16,
    },
    EchoRequestPacket,
    NetworkError,
    IdenticalRequests {
        host: IpAddr,
        ident: Option<u16>,
        seq: u16,
    },
    ClientDestroyed,
}

impl From<surge_ping::SurgeError> for PingFailure {
    fn from(value: surge_ping::SurgeError) -> Self {
        match value {
            surge_ping::SurgeError::IncorrectBufferSize => Self::IncorrectBufferSize,
            surge_ping::SurgeError::MalformedPacket(_) => Self::MalformedPacket,
            surge_ping::SurgeError::IOError(error) => Self::Io(error.kind()),
            surge_ping::SurgeError::Timeout { seq } => Self::Timeout {
                seq: seq.into_u16(),
            },
            surge_ping::SurgeError::EchoRequestPacket => Self::EchoRequestPacket,
            surge_ping::SurgeError::NetworkError => Self::NetworkError,
            surge_ping::SurgeError::IdenticalRequests { host, ident, seq } => {
                Self::IdenticalRequests {
                    host,
                    ident: ident.map(|id| id.into_u16()),
                    seq: seq.into_u16(),
                }
            }
            surge_ping::SurgeError::ClientDestroyed => Self::ClientDestroyed,
        }
    }
}

impl std::fmt::Display for PingFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::IncorrectBufferSize => f.write_str("buffer size was too small"),
            Self::MalformedPacket => f.write_str("malformed packet"),
            Self::Io(kind) => write!(f, "io error: {}", io::Error::from(*kind)),
            Self::Timeout { seq } => write!(f, "request timeout for icmp_seq {seq}"),
            Self::EchoRequestPacket => f.write_str("echo request packet"),
            Self::NetworkError => f.write_str("network error"),
            Self::IdenticalRequests { host, ident, seq } => match ident {
                Some(ident) => write!(
                    f,
                    "multiple identical request (host={host}, ident={ident}, seq={seq})"
                ),
                None => write!(f, "multiple identical request (host={host}, seq={seq})"),
            },
            Self::ClientDestroyed => {
                f.write_str("client has been destroyed, ping operations are no longer available")
            }
        }
    }
}

/// Every piece of information the printer task needs to update its display.
///
/// Workers never touch indicatif; they only construct and send these variants.
/// The printer owns the interpretation and rendering of each one.
#[derive(Debug)]
pub enum PingEvent {
    /// DNS resolution succeeded; the bar should show the resolved address.
    Resolved { idx: types::HostIdx, addr: IpAddr },
    /// DNS resolution failed; the bar should be finished with an error message.
    ResolutionFailed {
        idx: types::HostIdx,
        error: types::ResolveError,
    },
    /// A ping round-trip succeeded with the given latency.
    Success { idx: types::HostIdx, rtt: Duration },
    /// A ping failed; a persistent timestamped line should be printed above the bars.
    Failure {
        idx: types::HostIdx,
        error: PingFailure,
    },
}

impl PingEvent {
    pub fn idx(&self) -> types::HostIdx {
        match self {
            Self::Resolved { idx, .. }
            | Self::ResolutionFailed { idx, .. }
            | Self::Success { idx, .. }
            | Self::Failure { idx, .. } => *idx,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ping_failure_formats_timeout_without_allocating_in_worker() {
        assert_eq!(
            PingFailure::Timeout { seq: 42 }.to_string(),
            "request timeout for icmp_seq 42"
        );
    }
}
