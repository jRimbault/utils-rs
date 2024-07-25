fn main() {
    let file = std::env::args().nth(1).expect("I need an xml file");
    let file = std::fs::File::open(file).expect("couldn't open file");
    pretty_xml::to_writer(file, std::io::stdout()).unwrap();
    println!();
}
