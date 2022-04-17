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
    /// in seconds
    #[clap(short, long, default_value_t = 10)]
    interval: u64,
    /// in seconds
    #[clap(short, long, default_value_t = 24 * 60 * 60)]
    period: u64,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let (interval, timeout) = args.timings.to_duration();
    let mut map = IndexMap::new();
    loop {
        let start = chrono::Utc::now();
        let uptime = crossbeam::scope(|scope| {
            let (sender, receiver) = crossbeam::channel::bounded(0);
            scope.spawn(move |_| {
                for (i, uptime) in receiver.into_iter().enumerate() {
                    eprint!("{i:<6}{uptime:.2}%\r");
                    io::stderr().flush().unwrap();
                }
                eprintln!();
            });
            poll_for(
                (args.ip_address, args.port.get()),
                interval,
                timeout,
                sender,
            )
        })
        .unwrap();
        map.insert(start, uptime?);
        println!("{map:#?}");
    }
}

fn poll_for<A>(
    address: A,
    interval: Duration,
    timeout: Duration,
    sender: crossbeam::channel::Sender<Percent>,
) -> anyhow::Result<Percent>
where
    A: ToSocketAddrs,
    A: Clone,
{
    let start = Instant::now();
    loop {
        let mut list = BitVec::new();
        match TcpStream::connect(address.clone()) {
            Ok(_) => list.push(true),
            Err(_) => list.push(false),
        }
        let uptime = uptime_rate(&list)?;
        sender.send(uptime).unwrap();
        if start.elapsed() >= timeout {
            return Ok(uptime);
        }
        thread::sleep(interval);
    }
}

fn uptime_rate(list: &BitVec) -> Result<Percent, conv::GeneralErrorKind> {
    let successes = f64::value_from(list.iter_ones().count())?;
    let total = f64::value_from(list.len())?;
    Ok(Percent(successes / total))
}

/// Keep the original floating point value, display it as percentage.
#[derive(Clone, Copy)]
struct Percent(f64);

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

impl ArgsTimings {
    fn to_duration(&self) -> (Duration, Duration) {
        (
            Duration::from_secs(self.interval),
            Duration::from_secs(self.period),
        )
    }
}
