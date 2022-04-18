mod args;
mod wrappers;

use args::{Args, Timings};
use clap::Parser;
use indexmap::IndexMap;
use std::{
    io::{self, Write},
    net::{TcpStream, ToSocketAddrs},
    thread,
    time::Instant,
};
use wrappers::{Percent, Stats};

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
{
    let start = Instant::now();
    let mut list = Stats::new();
    let address = address.to_socket_addrs()?.next().unwrap();
    loop {
        if start.elapsed() >= timings.period {
            return Ok(list);
        }
        match TcpStream::connect_timeout(&address, timings.timeout()) {
            Ok(_) => list.add_success(),
            Err(_) => list.add_failure(),
        }
        let uptime = list.uptime_rate()?;
        sender.send(uptime).unwrap();
        thread::sleep(timings.interval);
    }
}
