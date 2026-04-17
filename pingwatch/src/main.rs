#[tokio::main(flavor = "current_thread")]
async fn main() -> anyhow::Result<()> {
    let args = pingwatch::cli::Args::parse(env!("CARGO_BIN_NAME"))?;
    // Race the run loop against Ctrl+C; restore the cursor on signal so
    // indicatif's hidden cursor doesn't outlive the process.
    tokio::select! {
        biased;
        result = pingwatch::run(args) => result,
        _ = tokio::signal::ctrl_c() => {
            let _ = console::Term::stdout().show_cursor();
            Ok(())
        }
    }
}
