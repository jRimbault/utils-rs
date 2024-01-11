use std::{
    fs::File,
    path::{Path, PathBuf},
};

use anyhow::Context;
use clap::Parser;
use qriter::qriter;

#[derive(Debug, Parser)]
struct Args {
    file: PathBuf,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let file = File::open(&args.file)?;
    let name = args
        .file
        .file_stem()
        .context("file name should have a stem")?;
    let out = Path::new(name);
    std::fs::create_dir_all(out)?;
    for (i, image) in qriter(file).enumerate() {
        let name = format!("{:02}-qrcode.png", i + 1);
        image
            .save(out.join(&name))
            .context(format!("writing {name:?}"))?
    }
    Ok(())
}

mod qriter {
    use image::ImageBuffer;
    use image::Luma;
    use qrcode::QrCode;
    use std;
    use std::io::BufReader;
    use std::io::Read;

    pub fn qriter<R>(reader: R) -> QrIter<R>
    where
        R: Read,
    {
        QrIter::new(reader)
    }

    #[derive(Debug)]
    pub struct QrIter<R> {
        buffer: [u8; 2048],
        reader: BufReader<R>,
    }

    impl<R> QrIter<R>
    where
        R: Read,
    {
        fn new(reader: R) -> Self {
            QrIter {
                buffer: [0; 2048],
                reader: BufReader::new(reader),
            }
        }
    }

    impl<R> Iterator for QrIter<R>
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
                    Err(_) => return None,
                };
                let code = QrCode::new(&mut self.buffer[..len]).ok()?;
                return Some(code.render::<Luma<u8>>().build());
            }
        }
    }
}
