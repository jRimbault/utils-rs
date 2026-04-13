use crate::types::HostIdx;
use std::{net::IpAddr, time::Duration};

/// Every piece of information the printer task needs to update its display.
///
/// Workers never touch indicatif; they only construct and send these variants.
/// The printer owns the interpretation and rendering of each one.
#[derive(Debug)]
pub enum PingEvent {
    /// DNS resolution succeeded; the bar should show the resolved address.
    Resolved { idx: HostIdx, addr: IpAddr },
    /// DNS resolution failed; the bar should be finished with an error message.
    ResolutionFailed { idx: HostIdx, error: String },
    /// The ICMP client could not be created; the bar should be finished.
    ClientError { idx: HostIdx, error: String },
    /// A ping round-trip succeeded with the given latency.
    Success { idx: HostIdx, rtt: Duration },
    /// A ping failed; a persistent timestamped line should be printed above the bars.
    Failure { idx: HostIdx, error: String },
}

impl PingEvent {
    pub fn idx(&self) -> HostIdx {
        match self {
            Self::Resolved { idx, .. }
            | Self::ResolutionFailed { idx, .. }
            | Self::ClientError { idx, .. }
            | Self::Success { idx, .. }
            | Self::Failure { idx, .. } => *idx,
        }
    }
}
