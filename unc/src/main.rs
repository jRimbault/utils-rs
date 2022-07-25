/// This program is useless on any platform but Windows
///
/// https://superuser.com/questions/29933/get-the-current-unc-path-from-a-local-path-in-powershell
///
/// The standard library `canonicalize` function returns the correct UNC path on Windows.

use std::{
    env,
    path::Path,
};

#[cfg(not(windows))]
fn main() -> Result<(), &'static str> {
    Err("this is useless")
}

#[cfg(windows)]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let unc = match (env::args_os().nth(1), env::current_dir()) {
        (Some(arg), _) => {
            let arg: &Path = arg.as_ref();
            arg.canonicalize()?
        },
        (None, Ok(cwd)) => cwd.canonicalize()?,
        _ => Err("no path given")?,
    };
    println!("{}", unc.display());
    Ok(())
}
