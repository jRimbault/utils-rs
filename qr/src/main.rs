use std::{
    fs::File,
    path::{Path, PathBuf},
};

use anyhow::Context;
use clap::Parser;
use qriter::QrFileEncoder;

#[derive(Debug, Parser)]
struct Args {
    /// File to encode as QR codes
    file: PathBuf,
}

fn main() -> anyhow::Result<()> {
    env_logger::builder()
        .format_timestamp(None)
        .filter_level(log::LevelFilter::Debug)
        .init();
    let args = Args::parse();
    let file = File::open(&args.file)?;
    let name = args
        .file
        .file_stem()
        .and_then(|n| n.to_str())
        .context("file name should have an utf8 stem")?;
    let out = Path::new(env!("CARGO_BIN_NAME"));
    std::fs::create_dir_all(out)?;
    let encoder = QrFileEncoder::new(file);
    for (i, image) in encoder.into_iter().enumerate() {
        let name = format!("{:02}-{name}.png", i + 1);
        log::info!("saved {name:?} to {out:?}");
        image
            .save(out.join(&name))
            .context(format!("writing {name:?}"))?;
    }
    Ok(())
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
                let code = QrCode::new(&mut self.buffer[..len]).ok()?;
                return Some(code.render::<Luma<u8>>().build());
            }
        }
    }
}
