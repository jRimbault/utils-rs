use clap::{Parser, Subcommand, ValueEnum};

#[derive(Debug, Parser)]
struct Cli {
    url: url::Url,
    #[command(subcommand)]
    action: Action,
}

#[derive(Debug, Subcommand)]
enum Action {
    Get { target: Target },
    Set { target: Target, value: String },
}

#[derive(Debug, Clone, ValueEnum)]
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
        Action::Get {
            target: Target::Host,
        } => {
            println!("{}", args.url.host().unwrap());
        }
        Action::Get {
            target: Target::Scheme,
        } => {
            println!("{}", args.url.scheme());
        }
        Action::Get {
            target: Target::Path,
        } => {
            println!("{}", args.url.path());
        }
        Action::Get {
            target: Target::Query,
        } => {
            println!("{}", args.url.query().unwrap());
        }
        Action::Set {
            target: Target::Host,
            value,
        } => {
            let mut url = args.url.clone();
            url.set_host(Some(value))?;
            println!("{url}");
        }
        Action::Set {
            target: Target::Scheme,
            value,
        } => {
            let mut url = args.url.clone();
            url.set_scheme(&value).unwrap();
            println!("{url}");
        }
        Action::Set {
            target: Target::Path,
            value,
        } => {
            let mut url = args.url.clone();
            url.set_path(&value);
            println!("{url}");
        }
        Action::Set {
            target: Target::Query,
            value,
        } => {
            let mut url = args.url.clone();
            url.set_query(Some(value));
            println!("{url}");
        }
    }
    Ok(())
}
