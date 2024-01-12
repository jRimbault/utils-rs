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
