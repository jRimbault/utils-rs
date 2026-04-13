#[tokio::main(flavor = "current_thread")]
async fn main() -> anyhow::Result<()> {
    pingwatch::setup_ctrlc_handler()?;
    pingwatch::run(pingwatch::cli::Args::parse(env!("CARGO_BIN_NAME"))?).await
}
