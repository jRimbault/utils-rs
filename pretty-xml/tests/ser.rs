use pretty_xml::PrettyXml;
use xml::{EmitterConfig, ParserConfig};

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
