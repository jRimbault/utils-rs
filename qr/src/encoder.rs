use image::{ImageBuffer, Luma};
use qrcode::QrCode;
use std::io::{BufReader, Read};

#[derive(Debug)]
pub struct QrFileEncoder<R> {
    name: String,
    reader: BufReader<R>,
}

impl<R> QrFileEncoder<R>
where
    R: Read,
{
    pub fn new(reader: R, name: &str) -> Self {
        QrFileEncoder {
            name: name.to_owned(),
            reader: BufReader::new(reader),
        }
    }
}

#[derive(Debug)]
pub struct QrFileEncoderIter<R> {
    buffer: [u8; 2048],
    qr: QrFileEncoder<R>,
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
            qr: self,
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
            let len = match self.qr.reader.read(&mut self.buffer) {
                Ok(0) => return None,
                Ok(len) => len,
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {
                    log::warn!(
                        "reading {:?} was interrupted, trying to continue",
                        self.qr.name
                    );
                    continue;
                }
                Err(error) => {
                    log::error!("couldn't read {:?} {error}", self.qr.name);
                    return None;
                }
            };
            let code = QrCode::new(&self.buffer[..len]).ok()?;
            return Some(code.render::<Luma<u8>>().build());
        }
    }
}
