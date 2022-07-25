use std::path::{Path, PathBuf};

// https://superuser.com/questions/29933/get-the-current-unc-path-from-a-local-path-in-powershell

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = match (std::env::args_os().nth(1), std::env::current_dir()) {
        (Some(path), _) => canonicalize(path),
        (None, Ok(path)) => canonicalize(path),
        _ => Err("no path given")?,
    }?;
    println!("{}", path.display());
    Ok(())
}

fn canonicalize<P: AsRef<Path>>(path: P) -> std::io::Result<PathBuf> {
    path.as_ref().canonicalize()
}
