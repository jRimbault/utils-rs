mod imp;

use std::collections::HashMap;

use clap::{Parser, Subcommand, ValueEnum};
use serde::Serialize;

#[derive(Debug, Parser)]
struct Cli {
    url: url::Url,
    #[command(subcommand)]
    action: Action,
    #[clap(short, long)]
    json: bool,
}

#[derive(Debug, Subcommand)]
enum Action {
    /// Parts of the url to obtain
    Get { targets: Vec<Target> },
    /// Parts of the url to update
    Set {
        #[clap(value_parser = clap::value_parser!(SetAction))]
        actions: Vec<SetAction>,
    },
}

#[derive(Debug, Clone)]
struct SetAction {
    target: Target,
    value: String,
}

#[derive(Debug, Clone, Hash, PartialEq, Eq, ValueEnum, Serialize)]
#[serde(rename_all = "camelCase")]
enum Target {
    Fragment,
    Host,
    Path,
    Port,
    Query,
    Scheme,
}

fn main() -> color_eyre::Result<()> {
    color_eyre::install()?;
    let args = Cli::parse();
    match &args.action {
        Action::Get { targets } => {
            let map: HashMap<_, _> = targets
                .into_iter()
                .map(|t| (t, t.fetch(&args.url)))
                .collect();
            if args.json {
                serde_json::to_writer_pretty(std::io::stdout().lock(), &map)?;
            } else {
                print!("{map:?}");
            }
        }
        Action::Set { actions } => {
            let mut url = args.url.clone();
            for action in actions {
                action.target.set(&mut url, &action.value);
            }
            if args.json {
                serde_json::to_writer_pretty(std::io::stdout().lock(), &url)?;
            } else {
                print!("{url}");
            }
        }
    }
    println!();
    Ok(())
}

impl Target {
    fn fetch(&self, url: &url::Url) -> Option<String> {
        match self {
            Target::Fragment => url.fragment().map(ToString::to_string),
            Target::Host => url.host_str().map(ToString::to_string),
            Target::Path => Some(url.path().to_owned()),
            Target::Port => url.port_or_known_default().map(|port| port.to_string()),
            Target::Query => url.query().map(ToString::to_string),
            Target::Scheme => Some(url.scheme().to_owned()),
        }
    }

    fn set(&self, url: &mut url::Url, value: &str) {
        match self {
            Target::Fragment => url.set_fragment(Some(value)),
            Target::Host => url.set_host(Some(value)).expect("setting host"),
            Target::Path => url.set_path(&value),
            Target::Port => url.set_port(value.parse().ok()).expect("setting port"),
            Target::Query => url.set_query(Some(value)),
            Target::Scheme => url.set_scheme(&value).expect("setting scheme"),
        }
    }
}
