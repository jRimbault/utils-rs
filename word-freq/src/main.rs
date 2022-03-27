mod counter;

use anyhow::{Context, Result};
use clap::Parser;
use counter::SortedCounter;
use crossbeam_channel::Sender;
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
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
    #[clap(short = 'P', long, default_value_t = rayon::current_num_threads())]
    parallelism: usize,
}

fn main() -> Result<()> {
    let args = Args::parse();
    let SortedCounter(counter) = word_count_all(&args);
    let stdout = io::stdout();
    let mut stdout = stdout.lock();
    for (word, n) in counter.into_sorted_iter().take(args.top) {
        writeln!(stdout, "{word:?} {n}")?;
    }
    Ok(())
}

fn word_count_all(args: &Args) -> SortedCounter<String> {
    // there is always at least two threads, the one counting the items
    // and the one spawning the readers.
    // I'm only limiting the amount of parallel readers
    rayon::scope(|scope| {
        let (sender, receiver) = crossbeam_channel::bounded(32);
        scope.spawn(move |_| {
            let pool = rayon::ThreadPoolBuilder::new()
                .num_threads(args.parallelism)
                .build()
                .unwrap();
            pool.install(|| {
                args.paths.par_iter().for_each_with(sender, |sender, path| {
                    if let Err(error) = word_count_file(sender, args, path) {
                        eprintln!("Error: {error:?}");
                    }
                });
            });
        });
        receiver.into_iter().collect()
    })
}

fn word_count_file(sender: &mut Sender<String>, args: &Args, path: &Path) -> Result<()> {
    let case_sensitivity = |word: &str| {
        if args.case_insensitive {
            word.to_lowercase()
        } else {
            word.to_owned()
        }
    };
    let file = File::open(&path).with_context(|| format!("opening `{}`", path.display()))?;
    let file = BufReader::new(file);
    for line in file.lines() {
        let line = line.with_context(|| format!("reading from file `{}`", path.display()))?;
        for word in args.pattern.find_iter(&line) {
            let word = case_sensitivity(word.as_str());
            sender.send(word).unwrap();
        }
    }
    Ok(())
}
