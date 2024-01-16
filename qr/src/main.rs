use std::{fs::File, path::PathBuf, sync::mpsc::sync_channel};

mod encoder;

use anyhow::{Context, Result};
use clap::Parser;
use clap_verbosity_flag::{Verbosity, WarnLevel};
use encoder::QrFileEncoder;

#[derive(Debug, Parser)]
#[clap(author, version)]
struct Args {
    /// Files to encode as QR codes
    files: Vec<PathBuf>,
    /// Output directory
    #[arg(short, long, default_value = env!("CARGO_BIN_NAME"))]
    out: PathBuf,
    /// Open the QR codes automatically with your configured image viewer
    #[arg(long)]
    open: bool,
    #[command(flatten)]
    verbose: Verbosity<WarnLevel>,
}

fn main() -> Result<()> {
    let args = Args::parse();
    env_logger::builder()
        .filter_level(args.verbose.log_level_filter())
        .init();
    run(&args)
}

fn run(args: &Args) -> Result<()> {
    std::thread::scope(|scope| {
        let (sender, receiver) = sync_channel(1);
        for file in &args.files {
            let name = file
                .file_name()
                .and_then(|n| n.to_str())
                .context("file name should be utf8")?;
            let file = File::open(&file)?;
            let sender = sender.clone();
            scope.spawn(move || {
                let encoder = QrFileEncoder::new(file);
                for (i, image) in encoder.into_iter().enumerate() {
                    log::debug!("encoded part of {name}");
                    sender
                        .send((format!("{:02}-{name}.png", i + 1), image))
                        .unwrap();
                }
            });
        }
        drop(sender);
        let out = &args.out;
        std::fs::create_dir_all(out)?;
        for (name, image) in receiver {
            let path = out.join(&name);
            image.save(&path).context(format!("writing {name:?}"))?;
            log::info!("saved {name:?} to {out:?}");
            if args.open {
                open::that_detached(path)?;
            }
        }
        Ok(())
    })
}
