//! Recursively finds Git/Mercurial/Jujutsu repositories beneath a given path.
//! Designed for speed in interactive pipelines like: cd $(find_projects | fzf)
//!
//! Performance characteristics:
//! - Uses a simple single-threaded `walkdir` traversal
//! - Results are sorted for stable, predictable output across invocations
//! - Stops descending into repository roots to avoid redundant traversal
use clap::Parser;
use std::{
    io::{BufWriter, Write},
    path::PathBuf,
};
use walkdir::{DirEntry, WalkDir};

/// Recursively list Git / Mercurial repositories beneath PATH (defaults to CWD)
#[derive(Parser)]
struct Args {
    /// Base directory to start the search
    #[arg(value_name = "PATH", default_value = ".")]
    path: PathBuf,
}

fn main() -> std::io::Result<()> {
    let args = Args::parse();
    let mut results = Vec::new();
    let mut walker = WalkDir::new(&args.path).into_iter();
    while let Some(entry) = walker.next() {
        let Ok(entry) = entry else {
            continue;
        };
        if is_repo_root(&entry) {
            if let Ok(relative) = entry.path().strip_prefix(&args.path) {
                results.push(relative.to_path_buf());
            }
            walker.skip_current_dir();
        }
    }
    results.sort();
    let mut stdout = BufWriter::new(std::io::stdout().lock());
    for r in results {
        writeln!(&mut stdout, "{}", r.display())?;
    }
    Ok(())
}

fn is_repo_root(dir: &DirEntry) -> bool {
    let path = dir.path();
    // Fast checks: ignore files; we only care about directories
    if !dir.file_type().is_dir() {
        return false;
    }
    path.join(".git").is_dir() || path.join(".hg").is_dir() || path.join(".jj").is_dir()
}
