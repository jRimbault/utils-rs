use std::{
    fs::File,
    path::{Path, PathBuf},
};

mod encoder;

use anyhow::{Context, Result};
use clap::Parser;
use clap_verbosity_flag::{Verbosity, WarnLevel};
use encoder::QrFileEncoder;

#[derive(Debug, Parser)]
struct Args {
    /// File to encode as QR codes
    file: PathBuf,
    #[command(flatten)]
    verbose: Verbosity<WarnLevel>,
}

fn main() -> Result<()> {
    let args = Args::parse();
    env_logger::builder()
        .filter_level(args.verbose.log_level_filter())
        .init();
    let file = File::open(&args.file)?;
    let name = args
        .file
        .file_name()
        .and_then(|n| n.to_str())
        .context("file name should be utf8")?;
    std::thread::scope(|scope| -> Result<()> {
        let (sender, receiver) = std::sync::mpsc::sync_channel(1);
        scope.spawn(move || {
            let encoder = QrFileEncoder::new(file);
            for (i, image) in encoder.into_iter().enumerate() {
                log::debug!("encoded part of {name}");
                sender
                    .send((format!("{:02}-{name}.png", i + 1), image))
                    .unwrap();
            }
        });
        let out = Path::new(env!("CARGO_BIN_NAME"));
        std::fs::create_dir_all(out)?;
        for (name, image) in receiver {
            image
                .save(out.join(&name))
                .context(format!("writing {name:?}"))?;
            log::info!("saved {name:?} to {out:?}");
        }
        Ok(())
    })
}
