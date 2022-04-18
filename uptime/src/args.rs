use std::{net::IpAddr, num::NonZeroU16, time::Duration};

use clap::Parser;

/// Get a running uptime
#[derive(Parser, Debug)]
#[clap(author, version)]
pub struct Args {
    /// server to poll
    pub ip_address: IpAddr,
    /// port to poll 1-65536
    #[clap(default_value_t = unsafe { NonZeroU16::new_unchecked(80) })]
    pub port: NonZeroU16,
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
        (self.interval / 2)
            .max(Timings::MIN_TIMEOUT)
            .min(Timings::MAX_TIMEOUT)
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
