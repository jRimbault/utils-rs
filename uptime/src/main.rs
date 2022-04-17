use bitvec::vec::BitVec;
use clap::Parser;
use conv::ValueFrom;
use indexmap::IndexMap;
use std::{
    io::{self, Write},
    net::{IpAddr, TcpStream, ToSocketAddrs},
    num::NonZeroU16,
    thread,
    time::{Duration, Instant},
};

/// Get a running uptime
#[derive(Parser, Debug)]
#[clap(author, version)]
struct Args {
    /// server to poll
    ip_address: IpAddr,
    /// port to poll 1-65536
    #[clap(default_value_t = unsafe { NonZeroU16::new_unchecked(80) })]
    port: NonZeroU16,
    #[clap(flatten)]
    timings: ArgsTimings,
}

#[derive(Parser, Debug)]
struct ArgsTimings {
    /// in milliseconds
    #[clap(short, long, default_value_t = 10_000)]
    interval: u64,
    /// in seconds
    #[clap(short, long, default_value_t = 24 * 60 * 60)]
    period: u64,
}

#[derive(Debug, Clone, Copy)]
struct Timings {
    interval: Duration,
    period: Duration,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let timings = args.timings.to_duration();
    let mut map = IndexMap::new();
    loop {
        let start = chrono::Utc::now();
        let stats = crossbeam::scope(|scope| {
            let (sender, receiver) = crossbeam::channel::bounded(0);
            scope.spawn(move |_| {
                for (i, uptime) in receiver.into_iter().enumerate() {
                    eprint!("{:<6}{uptime:.2}%\r", i + 1);
                    io::stderr().flush().unwrap();
                }
                eprint!("             \r");
                io::stderr().flush().unwrap();
            });
            poll((args.ip_address, args.port.get()), timings, sender)
        })
        .unwrap()?;
        println!(
            "{start}: {stats:>10?} [{}/{} tests]",
            stats.successes(),
            stats.len()
        );
        map.insert(start, stats);
    }
}

fn poll<A>(
    address: A,
    timings: Timings,
    sender: crossbeam::channel::Sender<Percent>,
) -> anyhow::Result<Stats>
where
    A: ToSocketAddrs,
    A: Clone,
{
    let start = Instant::now();
    let mut list = BitVec::new();
    let address = address.to_socket_addrs()?.next().unwrap();
    loop {
        if start.elapsed() >= timings.period {
            return Ok(Stats(list));
        }
        match TcpStream::connect_timeout(&address, timings.interval / 2) {
            Ok(_) => list.push(true),
            Err(_) => list.push(false),
        }
        let uptime = uptime_rate(&list)?;
        sender.send(uptime).unwrap();
        thread::sleep(timings.interval);
    }
}

fn uptime_rate(list: &BitVec) -> Result<Percent, conv::GeneralErrorKind> {
    let successes = f64::value_from(list.count_ones())?;
    let total = f64::value_from(list.len())?;
    Ok(Percent(successes / total))
}

/// Keep the original floating point value, display it as percentage.
#[derive(Clone, Copy)]
struct Percent(f64);

struct Stats(BitVec);

impl Stats {
    fn uptime_rate(&self) -> Result<Percent, conv::GeneralErrorKind> {
        uptime_rate(&self.0)
    }
    fn successes(&self) -> usize {
        self.0.count_ones()
    }
    fn len(&self) -> usize {
        self.0.len()
    }
}

impl ArgsTimings {
    fn to_duration(&self) -> Timings {
        Timings {
            interval: Duration::from_millis(self.interval),
            period: Duration::from_secs(self.period),
        }
    }
}

mod wrappers {
    use super::{Percent, Stats};

    impl std::fmt::Display for Percent {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            (self.0 * 100.).fmt(f)
        }
    }
    impl std::fmt::Debug for Percent {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            self.0.fmt(f)
        }
    }

    impl std::fmt::Debug for Stats {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            let rate = self.uptime_rate().map_err(|_| std::fmt::Error)?;
            write!(f, "{:.2}%", rate)
        }
    }
}
