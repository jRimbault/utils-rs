use color_eyre::{eyre::WrapErr, Help, SectionExt};
use std::net::IpAddr;

/// This program helps getting your WAN address
///
/// It uses ipify.org API.
#[derive(Debug, clap::Parser)]
#[clap(version, author)]
enum Ip {
    /// get your WAN IPv4 address
    V4,
    /// get your WAN IPv6 address
    V6,
}

fn main() -> color_eyre::eyre::Result<()> {
    color_eyre::install()?;
    let ip: Ip = clap::Parser::parse();
    let address = ip.get().wrap_err("getting your IP address")?;
    println!("{address}");
    Ok(())
}

impl Ip {
    fn get(&self) -> color_eyre::eyre::Result<IpAddr> {
        let url = match self {
            Ip::V4 => "https://api.ipify.org/?format=text",
            Ip::V6 => "https://api64.ipify.org/?format=text",
        };
        let response = ureq::get(url).call().wrap_err("calling the ipify API")?;
        let response = response
            .into_string()
            .wrap_err("converting ipify response into an UTF-8 string")?;
        let ip = response
            .trim()
            .parse()
            .wrap_err("parsing response")
            .section(response.header("ipify response body"))?;
        Ok(ip)
    }
}
