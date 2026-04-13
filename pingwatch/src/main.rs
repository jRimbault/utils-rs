use clap::Parser;

#[tokio::main(flavor = "current_thread")]
async fn main() -> anyhow::Result<()> {
    pingwatch::run(pingwatch::cli::Args::parse()).await
}
