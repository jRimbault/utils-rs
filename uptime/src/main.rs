mod args;
mod wrappers;

use args::{Args, Timings};
use chrono::SecondsFormat;
use clap::Parser;
use crossbeam_channel as channel;
use indexmap::IndexMap;
use std::{
    io::{self, Write},
    net::{SocketAddr, TcpStream, ToSocketAddrs},
};
use wrappers::{RollingStats, Stats};

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let timings = args.timings.to_duration();
    let address = (args.ip_address, args.port.get())
        .to_socket_addrs()?
        .next()
        .ok_or_else(|| anyhow::anyhow!("there should be at least 1 address here"))?;
    rayon::scope(|scope| {
        let (progress_tx, progress_rx) = channel::bounded(0);
        scope.spawn(move |_| report(progress_rx, timings.intervals()));
        let mut map = IndexMap::new();
        loop {
            let start = chrono::Utc::now();
            let results = poll(scope, address, timings, progress_tx.clone())?;
            println!(
                "{}: {:>6.2}% [{}/{} tests]",
                start.to_rfc3339_opts(SecondsFormat::Secs, true),
                results.success_rate()?,
                results.successes(),
                results.len()
            );
            map.insert(start, results);
        }
    })
}

fn poll(
    scope: &rayon::Scope,
    address: SocketAddr,
    timings: Timings,
    progress_tx: channel::Sender<Option<bool>>,
) -> anyhow::Result<Stats> {
    let (poll_tx, poll_rx) = channel::bounded(0);
    let (stats_tx, stats_rx) = channel::bounded(0);
    scope.spawn(move |_| {
        if let Err(error) = count_results(poll_rx, progress_tx, stats_tx) {
            eprintln!("{error}");
        }
    });
    let ticker = ticker::Ticker::builder()
        .limit(timings.period)
        .interval(timings.interval)
        .build()?;
    for _ in ticker {
        let poll_tx = poll_tx.clone();
        scope.spawn(move |_| {
            if let Err(error) = try_connect(poll_tx, address, timings) {
                eprintln!("{error}");
            }
        });
    }
    drop(poll_tx);
    Ok(stats_rx.recv()?)
}

fn try_connect(
    poll_tx: channel::Sender<Result<TcpStream, io::Error>>,
    address: std::net::SocketAddr,
    timings: Timings,
) -> anyhow::Result<()> {
    poll_tx.send(TcpStream::connect_timeout(&address, timings.timeout()))?;
    Ok(())
}

fn count_results<T, E>(
    results_rx: channel::Receiver<Result<T, E>>,
    progress_tx: channel::Sender<Option<bool>>,
    stats_tx: channel::Sender<Stats>,
) -> anyhow::Result<()> {
    let mut list = Stats::new();
    for result in results_rx {
        match result {
            Ok(_) => list.add_success(),
            Err(_) => list.add_failure(),
        }
        progress_tx.send(Some(result.is_ok()))?;
    }
    progress_tx.send(None)?;
    stats_tx.send(list)?;
    Ok(())
}

fn report(progress_rx: channel::Receiver<Option<bool>>, intervals: usize) {
    let mut rolling = RollingStats::with_capacity(intervals);
    loop {
        for (i, result) in progress_rx
            .clone()
            .into_iter()
            .take_while(Option::is_some)
            .flatten()
            .enumerate()
        {
            rolling.add(result);
            eprint!("{:>7} {:>6.2}%\r", i + 1, rolling.success_rate().unwrap());
            io::stderr().flush().unwrap();
        }
        eprint!("             \r");
        io::stderr().flush().unwrap();
    }
}

#[cfg(test)]
mod test {
    use super::{channel, count_results};

    #[test]
    fn result_counting() -> anyhow::Result<()> {
        let stats = rayon::scope(|scope| {
            let (progress_tx, progress_rx) = channel::unbounded();
            let (results_tx, results_rx) = channel::unbounded();
            let (stats_tx, stats_rx) = channel::unbounded();
            scope.spawn(move |_| progress_rx.into_iter().for_each(|_i| ()));
            scope.spawn(move |_| {
                let _ = count_results(results_rx, progress_tx, stats_tx);
            });
            for i in 0..10 {
                results_tx.send(Ok(i)).unwrap();
                results_tx.send(Err(i)).unwrap();
            }
            drop(results_tx);
            stats_rx.recv()
        })?;
        assert_eq!(stats.len(), 20);
        assert_eq!(stats.successes(), 10);
        Ok(())
    }
}
