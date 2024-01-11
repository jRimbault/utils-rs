use std::{
    fs::File,
    io::{BufReader, Read},
    path::{Path, PathBuf},
};

use anyhow::Context;
use clap::Parser;
use qrcode::QrCode;

#[derive(Debug, Parser)]
struct Args {
    file: Option<PathBuf>,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    if let Some(file_path) = args.file {
        let file = File::open(&file_path)?;
        let out = file_path.file_stem().unwrap();
        std::fs::create_dir_all(out).context("mkdir out")?;
        qr(file, out.as_ref())?;
    } else {
        std::fs::create_dir_all("out").context("mkdir out")?;
        qr(std::io::stdin().lock(), "out".as_ref())?;
    }
    Ok(())
}

fn qr<R>(reader: R, out: &Path) -> anyhow::Result<()>
where
    R: Read,
{
    let mut reader = BufReader::new(reader);
    let mut buffer = [0u8; 2048];
    let mut i = 1;
    loop {
        let len = match reader.read(&mut buffer) {
            Ok(0) => break,
            Ok(len) => len,
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(e)?,
        };
        let code = QrCode::new(&buffer[..len])?;
        let image = code.render::<image::Luma<u8>>().build();
        let name = format!("{:02}-qrcode.png", i);
        image
            .save(out.join(&name))
            .context(format!("writing {name:?}"))?;
        i += 1;
    }
    Ok(())
}
