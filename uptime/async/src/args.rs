use clap::Parser;
use std::{net::SocketAddr, time::Duration};

/// Get a running uptime
#[derive(Parser, Debug)]
#[clap(author, version)]
pub struct Args {
    /// address to poll (127.0.0.1:80)
    pub address: SocketAddr,
    #[clap(flatten)]
    pub timings: ArgsTimings,
}

#[derive(Parser, Debug)]
pub struct ArgsTimings {
    /// in milliseconds
    #[clap(short, long, default_value_t = 10_000)]
    interval: u64,
    /// in seconds
    #[clap(short, long, default_value_t = 24 * 60 * 60)]
    period: u64,
}

#[derive(Debug, Default, Clone, Copy)]
pub struct Timings {
    pub interval: Duration,
    pub period: Duration,
}

impl ArgsTimings {
    pub fn to_duration(&self) -> Timings {
        Timings {
            interval: Duration::from_millis(self.interval),
            period: Duration::from_secs(self.period),
        }
    }
}

impl Timings {
    const MIN_TIMEOUT: Duration = Duration::from_millis(10);
    const MAX_TIMEOUT: Duration = Duration::from_millis(500);

    pub fn timeout(&self) -> Duration {
        self.interval
            .max(Timings::MIN_TIMEOUT)
            .min(Timings::MAX_TIMEOUT)
    }

    pub fn intervals(self) -> usize {
        let r = self.period.as_nanos() / self.interval.as_nanos();
        r as usize
    }
}

#[cfg(test)]
mod tests {
    use super::Timings;
    use std::time::Duration;

    #[test]
    fn timemout_constraints() {
        let list: Vec<_> = (0..40).map(|i| i * i).collect();
        for item in list {
            let t = Timings {
                interval: Duration::from_millis(item),
                ..Default::default()
            };
            assert!(t.timeout() <= Timings::MAX_TIMEOUT);
            assert!(t.timeout() >= Timings::MIN_TIMEOUT);
        }
    }
}
