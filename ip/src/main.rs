use anyhow::Context;
use clap::Parser;
use std::net::IpAddr;

/// This program helps getting your WAN address
///
/// It uses ipify.org API.
#[derive(Debug, Parser)]
#[clap(version, author)]
enum Ip {
    /// get your WAN IPv4 address
    V4,
    /// get your WAN IPv6 address
    V6,
}

fn main() -> anyhow::Result<()> {
    let ip = Ip::parse();
    let address = ip.get().context("getting your IP address")?;
    println!("{address}");
    Ok(())
}

impl Ip {
    fn get(&self) -> anyhow::Result<IpAddr> {
        let url = match self {
            Ip::V4 => "https://api.ipify.org/?format=text",
            Ip::V6 => "https://api64.ipify.org/?format=text",
        };
        Ok(reqwest::blocking::get(url)?.text()?.trim().parse()?)
    }
}
