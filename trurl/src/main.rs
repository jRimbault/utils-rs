mod imp;

use clap::{Parser, Subcommand, ValueEnum};
use itertools::Itertools;

#[derive(Debug, Parser)]
struct Cli {
    url: url::Url,
    #[command(subcommand)]
    action: Action,
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

#[derive(Debug, Clone, Hash, PartialEq, Eq, ValueEnum)]
enum Target {
    Host,
    Path,
    Query,
    Scheme,
}

fn main() -> color_eyre::Result<()> {
    color_eyre::install()?;
    let args = Cli::parse();
    match &args.action {
        Action::Get { targets } => {
            for target in Itertools::intersperse(
                targets
                    .into_iter()
                    .unique()
                    .map(|target| target.fetch(&args.url)),
                Some(" "),
            ) {
                print!("{}", target.unwrap());
            }
            println!()
        }
        Action::Set { actions } => {
            let mut url = args.url.clone();
            for action in actions {
                action.target.set(&mut url, &action.value);
            }
            println!("{url}");
        }
    }
    Ok(())
}

impl Target {
    fn fetch<'a>(&self, url: &'a url::Url) -> Option<&'a str> {
        match self {
            Target::Host => url.host_str(),
            Target::Path => Some(url.path()),
            Target::Query => url.query(),
            Target::Scheme => Some(url.scheme()),
        }
    }

    fn set(&self, url: &mut url::Url, value: &str) {
        match self {
            Target::Host => url.set_host(Some(value)).unwrap(),
            Target::Path => url.set_path(&value),
            Target::Query => url.set_query(Some(value)),
            Target::Scheme => url.set_scheme(&value).unwrap(),
        }
    }
}
