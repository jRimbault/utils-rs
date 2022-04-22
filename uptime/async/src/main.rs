mod args;
mod wrappers;

use anyhow::Context;
use args::{Args, Timings};
use chrono::SecondsFormat;
use clap::Parser;
use indexmap::IndexMap;
use std::{io::Write, net::SocketAddr};
use tokio::sync::mpsc as channel;
use tokio::{net::TcpStream, time::Instant};
use wrappers::{RollingStats, Stats};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let timings = args.timings.to_duration();
    let (progress_tx, progress_rx) = tokio::sync::mpsc::channel(1);
    tokio::spawn(async move {
        if let Err(error) = report(progress_rx, timings.intervals()).await {
            eprintln!("Error while reporting results: {error}");
        }
    });
    let mut map = IndexMap::new();
    loop {
        let start = chrono::Utc::now();
        let results = poll(args.address, timings, progress_tx.clone()).await?;
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
) -> anyhow::Result<Stats> {
    let (poll_tx, poll_rx) = tokio::sync::mpsc::channel(1);
    let (stats_tx, mut stats_rx) = tokio::sync::mpsc::channel(1);
    tokio::spawn(async move {
        if let Err(error) = count_results(poll_rx, progress_tx, stats_tx).await {
            eprintln!("Error while counting results: {error}");
        }
    });
    let mut ticker = tokio::time::interval(timings.interval);
    let start = Instant::now();
    for _ in 0.. {
        ticker.tick().await;
        if start.elapsed() > timings.period {
            break;
        }
        let poll_tx = poll_tx.clone();
        tokio::spawn(async move {
            if let Err(error) = try_connect(poll_tx, address, timings).await {
                eprintln!("Error while trying to poll: {error}");
            }
        });
    }
    drop(poll_tx);
    Ok(stats_rx
        .recv()
        .await
        .ok_or_else(|| anyhow::anyhow!("didn't receive a final report"))?)
}

async fn try_connect(
    poll_tx: channel::Sender<Result<(), ()>>,
    address: std::net::SocketAddr,
    timings: Timings,
) -> anyhow::Result<()> {
    let timeout = timings.timeout();
    let result = tokio::time::timeout(timeout, TcpStream::connect(&address)).await;
    match result {
        Ok(Ok(_)) => poll_tx.send(Ok(())).await?,
        Ok(Err(_)) => poll_tx.send(Err(())).await?,
        Err(_) => poll_tx.send(Err(())).await?,
    }
    Ok(())
}

async fn count_results<T, E>(
    mut results_rx: channel::Receiver<Result<T, E>>,
    progress_tx: channel::Sender<Option<bool>>,
    stats_tx: channel::Sender<Stats>,
) -> anyhow::Result<()> {
    let mut list = Stats::new();
    for _ in 0.. {
        if let Some(result) = results_rx.recv().await {
            let result = result.is_ok();
            list.add(result);
            progress_tx
                .send(Some(result))
                .await
                .context("sending intermediate progress")?;
        } else {
            break;
        }
    }
    progress_tx.send(None).await?;
    stats_tx.send(list).await?;
    Ok(())
}

async fn report(
    mut progress_rx: channel::Receiver<Option<bool>>,
    intervals: usize,
) -> anyhow::Result<()> {
    let mut rolling = RollingStats::with_capacity(intervals);
    let mut stderr = std::io::stderr();
    loop {
        for i in 1.. {
            let result = progress_rx
                .recv()
                .await
                .ok_or_else(|| anyhow::anyhow!("waiting on intermediate progress"))?;
            let result = match result {
                Some(r) => r,
                None => break,
            };
            rolling.add(result);
            eprint!("{:>7} {:>6.2}%\r", i, rolling.success_rate()?);
            stderr.flush()?;
        }
        eprint!("             \r");
        stderr.flush()?;
    }
}
