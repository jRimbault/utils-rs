use std::{fmt, io};

use xml::{reader::ParserConfig, writer::EmitterConfig};

pub struct PrettyXml<'a>(pub &'a [u8]);

#[derive(Debug)]
pub enum Error {
    Read(xml::reader::Error),
    Write(xml::writer::Error),
}

impl From<xml::reader::Error> for Error {
    fn from(e: xml::reader::Error) -> Self {
        Error::Read(e)
    }
}

impl From<xml::writer::Error> for Error {
    fn from(e: xml::writer::Error) -> Self {
        Error::Write(e)
    }
}

pub fn to_writer<W>(writer: &mut W, buf: &[u8]) -> Result<usize, Error>
where
    W: io::Write,
{
    let reader = ParserConfig::new()
        .trim_whitespace(true)
        .ignore_comments(false)
        .create_reader(buf);
    let mut writer = EmitterConfig::new()
        .perform_indent(true)
        .normalize_empty_elements(false)
        .autopad_comments(false)
        .create_writer(writer);
    for event in reader {
        if let Some(event) = event?.as_writer_event() {
            writer.write(event)?;
        }
    }
    Ok(buf.len())
}

impl fmt::Display for PrettyXml<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        to_writer(&mut FmtWriter(f), self.0).map_err(|_| fmt::Error)?;
        Ok(())
    }
}

struct FmtWriter<'a, 'b>(&'b mut fmt::Formatter<'a>);

impl io::Write for FmtWriter<'_, '_> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let s = std::str::from_utf8(buf).map_err(to_io)?;
        fmt::Write::write_str(self.0, s).map_err(to_io)?;
        Ok(buf.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn to_io<E>(e: E) -> io::Error
where
    E: Into<Box<dyn std::error::Error + Send + Sync>>,
{
    io::Error::new(io::ErrorKind::Other, e)
}
