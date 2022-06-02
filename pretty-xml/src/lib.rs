use std::{fmt, io};

use xml::{reader::ParserConfig, writer::EmitterConfig};

pub struct PrettyXml<'a>(&'a [u8]);

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
        let s = std::str::from_utf8(buf).map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
        if fmt::Write::write_str(&mut self.0, s).is_err() {
            return Err(io::Error::new(
                io::ErrorKind::Other,
                "couldn't write to formatter",
            ));
        }
        Ok(buf.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::*;

    // from https://users.rust-lang.org/t/pretty-printing-xml/76372/3
    fn format_xml(src: &[u8]) -> Result<String, xml::reader::Error> {
        let mut dest = Vec::new();
        let reader = ParserConfig::new()
            .trim_whitespace(true)
            .ignore_comments(false)
            .create_reader(src);
        let mut writer = EmitterConfig::new()
            .perform_indent(true)
            .normalize_empty_elements(false)
            .autopad_comments(false)
            .create_writer(&mut dest);
        for event in reader {
            if let Some(event) = event?.as_writer_event() {
                writer.write(event).unwrap();
            }
        }
        Ok(String::from_utf8(dest).unwrap())
    }

    #[test]
    fn eq() {
        let xml = r##"<doc><i></i><i></i></doc>"##.as_bytes();
        assert_eq!(format_xml(xml).unwrap(), PrettyXml(xml).to_string());
    }
}
