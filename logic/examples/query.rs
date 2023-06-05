use std::{
    fs::File,
    io::{BufRead, BufReader, Write},
};

use anyhow::Context;
use logic::{CachedCircuit, Circuit};

fn main() -> anyhow::Result<()> {
    sanity();
    let circuit = Circuit::from_file("input.txt".as_ref()).context("parsing file")?;
    let circuit = CachedCircuit::from(circuit);
    println!("Ctrl-c to exit");
    loop {
        print!("Which cable to query: ");
        std::io::stdout().flush()?;
        let cable = input()?;
        match circuit.signal(&cable) {
            Some(signal) => println!("Cable {cable} has signal {signal}"),
            None => println!("error"),
        }
    }
}

fn input() -> std::io::Result<String> {
    let mut buf = String::new();
    std::io::stdin().read_line(&mut buf)?;
    buf.truncate(buf.trim_end().len());
    Ok(buf)
}

fn sanity() {
    let mut circuit = Circuit::default();
    let file = BufReader::new(File::open("input.txt").unwrap());
    for (lo, line) in file.lines().enumerate() {
        let line = line.unwrap();
        let lo = lo + 1;
        match circuit.add_connection(&line) {
            Ok(()) => {}
            Err(error) => {
                println!("{error} on l{lo} {line:?}");
            }
        }
    }
}
