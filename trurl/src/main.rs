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
}

fn main() {
    let args = Cli::parse();
    match args.action {
        Action::Get {
            target: Target::Host,
        } => {
            println!("{}", args.url.host().unwrap());
        }
        Action::Set {
            target: Target::Host,
            value,
        } => {
            let mut url = args.url.clone();
            url.set_host(Some(value).as_deref()).unwrap();
            println!("{url}");
        }
    }
}
