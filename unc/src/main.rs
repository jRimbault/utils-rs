/// This program is useless on any platform but Windows
///
/// https://superuser.com/questions/29933/get-the-current-unc-path-from-a-local-path-in-powershell
///
/// The standard library `canonicalize` function returns the correct UNC path on Windows.

use std::{
    env,
    io,
    path::{Path, PathBuf},
};

#[cfg(not(windows))]
fn main() -> Result<(), &'static str> {
    Err("this is useless")
}

#[cfg(windows)]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = match (env::args_os().nth(1), env::current_dir()) {
        (Some(path), _) => canonicalize(path),
        (None, Ok(path)) => canonicalize(path),
        _ => Err("no path given")?,
    }?;
    println!("{}", path.display());
    Ok(())
}

fn canonicalize<P: AsRef<Path>>(path: P) -> io::Result<PathBuf> {
    path.as_ref().canonicalize()
}
