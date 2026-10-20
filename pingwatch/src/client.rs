use std::net::IpAddr;

/// Shown when neither an unprivileged ICMP datagram socket nor a raw socket
/// could be opened, so the user learns how to grant the permission instead of
/// seeing a bare "Permission denied".
const PERMISSION_HINT: &str = "cannot open an ICMP socket; either allow unprivileged \
ping sockets for your group (sysctl -w net.ipv4.ping_group_range=\"0 2147483647\", \
persisted in /etc/sysctl.d/) or grant this binary CAP_NET_RAW \
(setcap cap_net_raw+ep <path-to-pingwatch>)";

/// ICMP clients, one per protocol version, handed out to workers.
///
/// Created once in [`crate::run`] and cloned into each worker task.
/// `surge_ping::Client` is `Arc`-backed internally, so cloning is cheap.
///
/// How workers get a client depends on the socket type the kernel granted:
///
/// - Raw sockets carry the echo identifier we choose, so replies are matched
///   by (address, identifier, seq) and every worker can share one socket.
/// - Linux ping sockets (`SOCK_DGRAM`, unprivileged) have the identifier
///   rewritten by the kernel to the socket's own value, so replies on a shared
///   socket are matched by (address, seq) only. Two hosts resolving to the same
///   address would then collide. Each worker gets its own socket instead, which
///   the kernel gives a distinct identifier.
#[derive(Clone)]
pub struct PingClients {
    inner: std::sync::Arc<Inner>,
}

struct Inner {
    v4: surge_ping::Client,
    v6: surge_ping::Client,
}

impl PingClients {
    pub fn new() -> anyhow::Result<Self> {
        Ok(Self {
            inner: std::sync::Arc::new(Inner {
                v4: open(surge_ping::ICMP::V4).map_err(with_permission_hint)?,
                v6: open(surge_ping::ICMP::V6).map_err(with_permission_hint)?,
            }),
        })
    }

    /// Returns the client a worker pinging `addr` should use for its lifetime.
    ///
    /// Must be called from within a tokio runtime: a fresh client spawns its
    /// own receive task.
    pub(crate) fn for_addr(&self, addr: IpAddr) -> surge_ping::Client {
        let (shared, kind) = match addr {
            IpAddr::V4(_) => (&self.inner.v4, surge_ping::ICMP::V4),
            IpAddr::V6(_) => (&self.inner.v6, surge_ping::ICMP::V6),
        };
        if !needs_dedicated_socket(shared.get_socket().get_type()) {
            return shared.clone();
        }
        // The shared socket already proved we have permission, so failure here
        // is resource exhaustion (e.g. file descriptor limit). Falling back to
        // the shared socket keeps the host monitored; it only risks collisions
        // with another host that resolves to the same address.
        open(kind).unwrap_or_else(|_| shared.clone())
    }
}

fn open(kind: surge_ping::ICMP) -> std::io::Result<surge_ping::Client> {
    surge_ping::Client::new(&surge_ping::Config::builder().kind(kind).build())
}

/// Whether replies on a socket of this type are matched without the echo
/// identifier, which is the case for Linux ping sockets.
fn needs_dedicated_socket(sock_type: socket2::Type) -> bool {
    surge_ping::is_linux_icmp_socket!(sock_type)
}

fn with_permission_hint(err: std::io::Error) -> anyhow::Error {
    let kind = err.kind();
    let err = anyhow::Error::from(err);
    if kind == std::io::ErrorKind::PermissionDenied {
        err.context(PERMISSION_HINT)
    } else {
        err.context("cannot open an ICMP socket")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn raw_sockets_are_shared() {
        assert!(!needs_dedicated_socket(socket2::Type::RAW));
    }

    #[test]
    fn datagram_sockets_are_dedicated_only_on_linux() {
        assert_eq!(
            needs_dedicated_socket(socket2::Type::DGRAM),
            cfg!(any(target_os = "linux", target_os = "android"))
        );
    }

    #[test]
    fn permission_denied_suggests_fixes() {
        let err = with_permission_hint(std::io::Error::from(std::io::ErrorKind::PermissionDenied));
        let msg = format!("{err:#}");
        assert!(msg.contains("ping_group_range"), "{msg}");
        assert!(msg.contains("cap_net_raw"), "{msg}");
    }

    #[test]
    fn other_errors_get_generic_context() {
        let err = with_permission_hint(std::io::Error::from(std::io::ErrorKind::OutOfMemory));
        let msg = format!("{err:#}");
        assert!(msg.starts_with("cannot open an ICMP socket:"), "{msg}");
        assert!(!msg.contains("setcap"), "{msg}");
    }
}
