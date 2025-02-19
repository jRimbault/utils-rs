use std::{
    env,
    path::{Path, PathBuf},
    sync::mpsc,
    thread,
};

use walkdir::WalkDir;

fn main() {
    let base = env::args()
        .nth(1)
        .map_or_else(|| env::current_dir().unwrap(), PathBuf::from);
    thread::scope(|s| {
        let (sender, receiver) = mpsc::channel();
        s.spawn(|| scan_for_repositories(&base, sender));
        for project in receiver {
            println!("{}", project.strip_prefix(&base).unwrap().display());
        }
    });
}

fn scan_for_repositories(base: &Path, sender: mpsc::Sender<PathBuf>) {
    let mut walker = WalkDir::new(base).into_iter();
    while let Some(entry) = walker.next() {
        let Ok(entry) = entry else {
            continue;
        };
        let entry = entry.into_path();
        if entry.join(".git").is_dir() || entry.join(".hg").is_dir() {
            sender.send(entry).unwrap();
            walker.skip_current_dir();
        }
    }
}
