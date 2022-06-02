use std::{fmt, io};

use xml::{reader::ParserConfig, writer::EmitterConfig};

pub struct PrettyXml<'a>(pub &'a [u8]);

impl fmt::Display for PrettyXml<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let reader = ParserConfig::new()
            .trim_whitespace(true)
            .ignore_comments(false)
            .create_reader(self.0);
        let mut writer = EmitterConfig::new()
            .perform_indent(true)
            .normalize_empty_elements(false)
            .autopad_comments(false)
            .create_writer(FmtWriter(f));
        for event in reader {
            if let Some(event) = event.map_err(|_| fmt::Error)?.as_writer_event() {
                writer.write(event).map_err(|_| fmt::Error)?;
            }
        }
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
