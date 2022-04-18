mod args;
mod wrappers;

use args::{Args, Timings};
use chrono::SecondsFormat;
use clap::Parser;
use indexmap::IndexMap;
use std::{
    io::{self, Write},
    net::{TcpStream, ToSocketAddrs},
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
                    eprint!("{:>6} {uptime:>6.2}%\r", i + 1);
                    io::stderr().flush().unwrap();
                }
                eprint!("             \r");
                io::stderr().flush().unwrap();
            });
            poll((args.ip_address, args.port.get()), timings, sender)
        })
        .unwrap()?;
        println!(
            "{}: {:>6.2}% [{}/{} tests]",
            start.to_rfc3339_opts(SecondsFormat::Secs, true),
            stats.uptime_rate()?,
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
    let address = address.to_socket_addrs()?.next().unwrap();
    let stats = crossbeam::scope(|scope| {
        let (tx, rx) = crossbeam::channel::bounded(0);
        let (stats_tx, stats_rx) = crossbeam::channel::bounded(0);
        scope.spawn(move |_| {
            let mut list = Stats::new();
            for result in rx {
                match result {
                    Ok(_) => list.add_success(),
                    Err(_) => list.add_failure(),
                }
                sender.send(list.uptime_rate().unwrap()).unwrap();
            }
            stats_tx.send(list).unwrap();
        });
        let start = Instant::now();
        crossbeam::channel::tick(timings.interval)
            .into_iter()
            .take_while(|_| start.elapsed() < timings.period)
            .for_each(move |_| {
                let tx = tx.clone();
                scope.spawn(move |_| {
                    tx.send(TcpStream::connect_timeout(&address, timings.timeout()))
                        .unwrap();
                });
            });
        stats_rx.recv().unwrap()
    })
    .unwrap();
    Ok(stats)
}
