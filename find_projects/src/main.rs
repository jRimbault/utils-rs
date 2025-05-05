// src/main.rs
use clap::Parser;
use ignore::{DirEntry, WalkBuilder, WalkState};
use std::{path::PathBuf, thread};

/// Recursively list Git / Mercurial repositories beneath PATH (defaults to CWD)
#[derive(Parser)]
struct Args {
    /// Base directory to start the search
    #[arg(value_name = "PATH", default_value = ".")]
    path: PathBuf,
}

fn main() {
    let args = Args::parse();
    let mut results: Vec<_> = thread::scope(|s| {
        let (sender, receiver) = crossbeam_channel::unbounded();
        let base = args.path.as_path();
        s.spawn(move || {
            // WalkBuilder is threaded by default; disable git-ignore handling
            // so **all** directories (even those ignored) are visited.
            WalkBuilder::new(base)
                .standard_filters(false)
                .threads(
                    std::thread::available_parallelism()
                        .map(|n| n.get())
                        .unwrap_or(2),
                )
                .build_parallel()
                .run(move || {
                    let sender = sender.clone();
                    Box::new(move |result| {
                        let entry = match result {
                            Ok(e) => e,
                            Err(_) => return WalkState::Continue, // ignore unreadable paths
                        };

                        if is_repo_root(&entry) {
                            sender.send(entry).unwrap();
                            // Tell the walker not to descend any further into this repository
                            WalkState::Skip
                        } else {
                            WalkState::Continue
                        }
                    })
                });
        });
        receiver
            .into_iter()
            .filter_map(|entry| {
                entry
                    .path()
                    .strip_prefix(&args.path)
                    .ok()
                    .map(PathBuf::from)
            })
            .collect()
    });
    results.sort();
    for r in results {
        println!("{}", r.display());
    }
}

fn is_repo_root(dir: &DirEntry) -> bool {
    let path = dir.path();
    // Fast checks: ignore files; we only care about directories
    if !dir.file_type().is_some_and(|ft| ft.is_dir()) {
        return false;
    }
    // Does this directory contain a '.git' or '.hg' child?
    // We look for **subdirectories** named exactly ".git" or ".hg".
    // Using std::fs::read_dir keeps us from descending further unless necessary.
    if let Ok(mut children) = std::fs::read_dir(path) {
        while let Some(Ok(child)) = children.next() {
            if child.file_type().is_ok_and(|ft| ft.is_dir()) {
                let name = child.file_name();
                if name == ".git" || name == ".hg" {
                    return true;
                }
            }
        }
    }
    false
}
