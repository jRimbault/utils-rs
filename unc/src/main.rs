//! This program is useless on any platform but Windows
//!
//! [See here for a script version.](https://superuser.com/questions/29933/get-the-current-unc-path-from-a-local-path-in-powershell)
//!
//! The standard library `canonicalize` function returns the correct UNC path on Windows.

#[cfg(not(windows))]
use unix as platform;
#[cfg(windows)]
use windows as platform;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    Ok(platform::main()?)
}

#[cfg(windows)]
mod windows {
    use std::{env, ffi::OsStr, io, path::Path};

    pub fn main() -> io::Result<()> {
        let help_flags = ["-h", "--help"].map(OsStr::new);
        let unc = match (env::args_os().nth(1), env::current_dir()) {
            (Some(arg), _) if help_flags.contains(&arg.as_os_str()) => {
                help();
                return Ok(());
            }
            (Some(arg), _) => Path::new(&arg).canonicalize()?,
            (None, Ok(cwd)) => cwd.canonicalize()?,
            _ => return fatal_error(),
        };
        println!("{}", unc.display());
        Ok(())
    }

    fn help() {
        println!("Give the UNC path of the given directory or of the current working directory.");
    }

    fn fatal_error() -> io::Result<()> {
        Err(io::Error::new(
            io::ErrorKind::Other,
            "You should either give an argument or be in a directory with correct permissions",
        ))
    }
}

#[cfg(not(windows))]
mod unix {
    pub fn main() -> Result<(), &'static str> {
        Err("This program is useless on any platform but Windows")
    }
}
