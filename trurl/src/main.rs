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
    Get { targets: Vec<Target> },
    Set { target: Target, value: String },
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
            for target in targets
                .into_iter()
                .unique()
                .map(|target| target.fetch(&args.url))
                .intersperse(Some(" "))
            {
                print!("{}", target.unwrap());
            }
            println!()
        }
        Action::Set { target, value } => {
            let url = target.set(&args.url, value);
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

    fn set(&self, url: &url::Url, value: &str) -> url::Url {
        let mut url = url.clone();
        match self {
            Target::Host => url.set_host(Some(value)).unwrap(),
            Target::Path => url.set_path(&value),
            Target::Query => url.set_query(Some(value)),
            Target::Scheme => url.set_scheme(&value).unwrap(),
        }
        url
    }
}
