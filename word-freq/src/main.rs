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
    #[clap(required = true, parse(from_os_str))]
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
    let counter = parallel_word_count(&args);
    let stdout = io::stdout();
    let mut stdout = stdout.lock();
    for (word, n) in counter.into_sorted_iter().take(args.top) {
        writeln!(stdout, "{word:?} {n}")?;
    }
    Ok(())
}

fn parallel_word_count(args: &Args) -> PriorityQueue<String, usize> {
    rayon::scope(|scope| {
        let (sender, receiver) = crossbeam_channel::unbounded();
        start_file_collecting(scope, args, sender);
        let mut counter = PriorityQueue::new();
        for word in receiver {
            let mut found = false;
            counter.change_priority_by(&word, |n| {
                found = true;
                *n += 1;
            });
            if !found {
                counter.push(word, 1);
            }
        }
        counter
    })
}

fn start_file_collecting<'scope>(
    scope: &rayon::Scope<'scope>,
    args: &'scope Args,
    sender: Sender<String>,
) {
    for path in &args.paths {
        let sender = sender.clone();
        let pattern = &args.pattern;
        scope.spawn(move |_| {
            if let Err(error) = word_count_file(sender, &path, pattern, args.case_insensitive) {
                eprintln!("Error: {error:?}");
            }
        });
    }
}

fn word_count_file(
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
