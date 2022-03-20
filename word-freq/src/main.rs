use anyhow::{Context, Result};
use clap::Parser;
use crossbeam_channel::Sender;
use priority_queue::PriorityQueue;
use regex::Regex;
use std::{
    fs::File,
    io::{self, BufRead, BufReader, Write},
    path::{Path, PathBuf},
};

#[derive(Debug, Parser)]
#[clap(author, version)]
struct Args {
    #[clap(parse(from_os_str))]
    paths: Vec<PathBuf>,
    #[clap(short, long, default_value_t = Regex::new(r#"\w+"#).unwrap())]
    pattern: Regex,
    #[clap(short = 'I', long)]
    case_insensitive: bool,
    #[clap(long, default_value_t = usize::MAX)]
    top: usize,
}

fn main() -> Result<()> {
    let args = Args::parse();
    let counter = rayon::scope(|scope| {
        let (sender, receiver) = crossbeam_channel::unbounded();
        for path in args.paths {
            let sender = sender.clone();
            let pattern = &args.pattern;
            scope.spawn(move |_| {
                if let Err(error) = file_count(sender, &path, pattern, args.case_insensitive) {
                    eprintln!("Error: {error:?}");
                }
            });
        }
        drop(sender);
        let mut counter = PriorityQueue::new();
        for word in receiver {
            let n = *counter.get_priority(&word).unwrap_or(&0_usize);
            counter.push(word, n + 1);
        }
        counter
    });
    let stdout = io::stdout();
    let mut stdout = stdout.lock();
    for (word, n) in counter.into_sorted_iter().take(args.top) {
        writeln!(stdout, "{word:?} {n}")?;
    }

    Ok(())
}

fn file_count(
    sender: Sender<String>,
    path: &Path,
    pattern: &Regex,
    case_insensitive: bool,
) -> Result<()> {
    let case_sensitivity = |word: &str| {
        if case_insensitive {
            word.to_lowercase()
        } else {
            word.to_owned()
        }
    };
    let file = File::open(&path).with_context(|| format!("opening `{}`", path.display()))?;
    let file = BufReader::new(file);
    for line in file.lines() {
        let line = line.with_context(|| format!("reading from file `{}`", path.display()))?;
        for word in pattern.find_iter(&line) {
            let word = case_sensitivity(word.as_str());
            sender.send(word).unwrap();
        }
    }
    Ok(())
}
