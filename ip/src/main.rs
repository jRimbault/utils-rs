use clap::Parser;
use std::net::IpAddr;

/// This program helps getting your WAN address
///
/// It uses ipify.org API.
#[derive(Debug, Parser)]
#[clap(version, author)]
enum Version {
    /// get your WAN IPv4 address
    V4,
    /// get your WAN IPv6 address
    V6,
}

fn main() -> anyhow::Result<()> {
    let version = Version::parse();
    let address: IpAddr = match version {
        Version::V4 => ip::v4()?.into(),
        Version::V6 => ip::v6()?.into(),
    };
    println!("{address:?}");
    Ok(())
}

mod ip {
    use std::net::{Ipv4Addr, Ipv6Addr};

    pub fn v4() -> anyhow::Result<Ipv4Addr> {
        Ok(get_text("https://api.ipify.org/?format=text")?
            .trim()
            .parse()?)
    }

    pub fn v6() -> anyhow::Result<Ipv6Addr> {
        Ok(get_text("https://api64.ipify.org/?format=text")?
            .trim()
            .parse()?)
    }

    fn get_text(url: &str) -> Result<String, reqwest::Error> {
        reqwest::blocking::get(url)?.text()
    }
}
