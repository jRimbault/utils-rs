use std::{
    fs::File,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use clap::Parser;
use clap_verbosity_flag::{Verbosity, WarnLevel};
use qriter::QrFileEncoder;

#[derive(Debug, Parser)]
struct Args {
    /// File to encode as QR codes
    file: PathBuf,
    /// Verbosity level
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
        .file_stem()
        .and_then(|n| n.to_str())
        .context("file name should have an utf8 stem")?;
    std::thread::scope(|scope| -> Result<()> {
        let (sender, receiver) = std::sync::mpsc::sync_channel(0);
        scope.spawn(move || {
            let encoder = QrFileEncoder::new(file);
            for (i, image) in encoder.into_iter().enumerate() {
                log::trace!("encoded part of {name}");
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

mod qriter {
    use image::{ImageBuffer, Luma};
    use qrcode::QrCode;
    use std::io::{BufReader, Read};

    #[derive(Debug)]
    pub struct QrFileEncoder<R> {
        reader: BufReader<R>,
    }

    impl<R> QrFileEncoder<R>
    where
        R: Read,
    {
        pub fn new(reader: R) -> Self {
            QrFileEncoder {
                reader: BufReader::new(reader),
            }
        }
    }

    #[derive(Debug)]
    pub struct QrFileEncoderIter<R> {
        buffer: [u8; 2048],
        reader: BufReader<R>,
    }

    impl<R> IntoIterator for QrFileEncoder<R>
    where
        R: Read,
    {
        type Item = ImageBuffer<Luma<u8>, Vec<u8>>;

        type IntoIter = QrFileEncoderIter<R>;

        fn into_iter(self) -> Self::IntoIter {
            QrFileEncoderIter {
                buffer: [0; 2048],
                reader: self.reader,
            }
        }
    }

    impl<R> Iterator for QrFileEncoderIter<R>
    where
        R: Read,
    {
        type Item = ImageBuffer<Luma<u8>, Vec<u8>>;

        fn next(&mut self) -> Option<Self::Item> {
            loop {
                let len = match self.reader.read(&mut self.buffer) {
                    Ok(0) => return None,
                    Ok(len) => len,
                    Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                    Err(error) => {
                        log::error!("couldn't read file {error}");
                        return None;
                    }
                };
                let code = QrCode::new(&self.buffer[..len]).ok()?;
                return Some(code.render::<Luma<u8>>().build());
            }
        }
    }
}
