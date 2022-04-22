mod args;
mod wrappers;

use args::{Args, Timings};
use chrono::SecondsFormat;
use clap::Parser;
use indexmap::IndexMap;
use std::{io::Write, net::SocketAddr};
use tokio::io;
use tokio::net::TcpStream;
use tokio::sync::mpsc as channel;
use wrappers::{RollingStats, Stats};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let timings = args.timings.to_duration();
    let (progress_tx, progress_rx) = tokio::sync::mpsc::channel(1);
    tokio::spawn(async move { report(progress_rx, timings.intervals()) });
    let mut map = IndexMap::new();
    loop {
        let start = chrono::Utc::now();
        let results = poll(args.address, timings, progress_tx.clone())
            .await
            .unwrap();
        println!(
            "{}: {} {:>6.2}% [{}/{} tests]",
            start.to_rfc3339_opts(SecondsFormat::Secs, true),
            args.address,
            results.success_rate()?,
            results.successes(),
            results.len(),
        );
        map.insert(start, results);
    }
}

async fn poll(
    address: SocketAddr,
    timings: Timings,
    progress_tx: channel::Sender<Option<bool>>,
) -> Option<Stats> {
    let (poll_tx, poll_rx) = tokio::sync::mpsc::channel(1);
    let (stats_tx, mut stats_rx) = tokio::sync::mpsc::channel(1);
    tokio::spawn(async {
        if let Err(error) = count_results(poll_rx, progress_tx, stats_tx).await {
            eprintln!("{error}");
        }
    });
    let mut ticker = tokio::time::interval(timings.interval);
    for _ in 0.. {
        ticker.tick().await;
        let poll_tx = poll_tx.clone();
        tokio::spawn(async move {
            if let Err(error) = try_connect(poll_tx, address, timings).await {
                eprintln!("{error}");
            }
        });
    }
    stats_rx.recv().await
}

async fn try_connect(
    poll_tx: channel::Sender<Result<TcpStream, io::Error>>,
    address: std::net::SocketAddr,
    timings: Timings,
) -> anyhow::Result<()> {
    let timeout = timings.timeout();
    let result = tokio::time::timeout(timeout, TcpStream::connect(&address)).await?;
    poll_tx.send(result).await?;
    Ok(())
}

async fn count_results<T, E>(
    mut results_rx: channel::Receiver<Result<T, E>>,
    progress_tx: channel::Sender<Option<bool>>,
    stats_tx: channel::Sender<Stats>,
) -> anyhow::Result<()> {
    let mut list = Stats::new();
    for _ in 0.. {
        let result = results_rx.recv().await.unwrap();
        let result = result.is_ok();
        list.add(result);
        progress_tx.send(Some(result)).await?;
    }
    progress_tx.send(None).await?;
    stats_tx.send(list).await?;
    Ok(())
}

async fn report(mut progress_rx: channel::Receiver<Option<bool>>, intervals: usize) {
    let mut rolling = RollingStats::with_capacity(intervals);
    loop {
        for i in 1.. {
            let result = progress_rx.recv().await.unwrap();
            let result = match result {
                Some(r) => r,
                None => break,
            };
            rolling.add(result);
            eprint!("{:>7} {:>6.2}%\r", i + 1, rolling.success_rate().unwrap());
            std::io::stderr().flush().unwrap();
        }
        eprint!("             \r");
        std::io::stderr().flush().unwrap();
    }
}
