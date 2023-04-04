mod imp;

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
    Password,
    Path,
    Port,
    Query,
    Scheme,
    User,
}

fn main() -> color_eyre::Result<()> {
    color_eyre::install()?;
    let args = Cli::parse();
    match &args.action {
        Action::Get { targets } => {
            let map = extract_to_json(&args.url, targets);
            if args.json {
                serde_json::to_writer_pretty(std::io::stdout().lock(), &map)?;
            } else {
                for (key, value) in map {
                    if key != "url" {
                        print!("{} ", value.as_str().unwrap());
                    }
                }
            }
        }
        Action::Set { actions } => {
            let mut url = args.url.clone();
            for action in actions {
                action.target.set(&mut url, &action.value);
            }
            if args.json {
                serde_json::to_writer_pretty(
                    std::io::stdout().lock(),
                    &extract_to_json(&url, Target::value_variants()),
                )?;
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
            Target::Password => url.password().map(ToString::to_string),
            Target::Path => Some(url.path().to_owned()),
            Target::Port => url.port_or_known_default().map(|port| port.to_string()),
            Target::Query => url.query().map(ToString::to_string),
            Target::Scheme => Some(url.scheme().to_owned()),
            Target::User => Some(url.username().to_owned()),
        }
    }

    fn set(&self, url: &mut url::Url, value: &str) {
        match self {
            Target::Fragment => url.set_fragment(Some(value)),
            Target::Host => url
                .set_host(Some(value))
                .unwrap_or_else(|_| panic!("invalid host: {value:?}")),
            Target::Password => url
                .set_password(Some(value))
                .unwrap_or_else(|_| panic!("invalid password: {value:?}")),
            Target::Path => url.set_path(value),
            Target::Port => url
                .set_port(value.parse().ok())
                .unwrap_or_else(|_| panic!("invalid port: {value:?}")),
            Target::Query => url.set_query(Some(value)),
            Target::Scheme => url
                .set_scheme(value)
                .unwrap_or_else(|_| panic!("invalid scheme: {value:?}")),
            Target::User => url
                .set_username(value)
                .unwrap_or_else(|_| panic!("invalid user: {value:?}")),
        }
    }
}

fn extract_to_json(url: &url::Url, parts: &[Target]) -> serde_json::Map<String, serde_json::Value> {
    let mut map = serde_json::Map::new();
    map.insert("url".into(), url.to_string().into());
    for part in parts {
        if let Some(value) = part.fetch(url) {
            if value.is_empty() {
                continue;
            }
            map.insert(part.to_string(), value.into());
        }
    }
    map
}
