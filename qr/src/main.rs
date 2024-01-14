use std::{fs::File, path::PathBuf, sync::mpsc::sync_channel};

mod encoder;

use anyhow::{Context, Result};
use clap::Parser;
use clap_verbosity_flag::{Verbosity, WarnLevel};
use encoder::QrFileEncoder;

#[derive(Debug, Parser)]
struct Args {
    /// File to encode as QR codes
    file: PathBuf,
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
    let file = File::open(&args.file)?;
    let name = args
        .file
        .file_name()
        .and_then(|n| n.to_str())
        .context("file name should be utf8")?;
    std::thread::scope(|scope| {
        let (sender, receiver) = sync_channel(1);
        scope.spawn(move || {
            let encoder = QrFileEncoder::new(file);
            for (i, image) in encoder.into_iter().enumerate() {
                log::debug!("encoded part of {name}");
                sender
                    .send((format!("{:02}-{name}.png", i + 1), image))
                    .unwrap();
            }
        });
        let out = &args.out;
        std::fs::create_dir_all(out)?;
        let mut images = Vec::new();
        for (name, image) in receiver {
            let path = out.join(&name);
            image.save(&path).context(format!("writing {name:?}"))?;
            log::info!("saved {name:?} to {out:?}");
            if args.open {
                images.push(path);
            }
        }
        for image in images {
            open::that_detached(image)?;
        }
        Ok(())
    })
}
