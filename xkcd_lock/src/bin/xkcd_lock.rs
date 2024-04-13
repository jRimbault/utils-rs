use std::{path::Path, process::Command};

fn main() -> anyhow::Result<()> {
    env_logger::init();
    let comic = xkcd_lock::comic::Xkcd::random()?;
    let file = comic.download()?;
    let file = comic.write_to_file_as_bg(&file)?;
    swaylock(&file)
}

fn swaylock(file: &Path) -> anyhow::Result<()> {
    let background =
        std::env::var("BG_LOCK_IMAGE").expect("you should have set a BG_LOCK_IMAGE env variable");
    let displays = xkcd_lock::displays()?;
    let mut all_monitors = vec![
        "-i".to_owned(),
        format!("{}:{}", displays[0], file.to_string_lossy()),
    ];
    all_monitors.extend(
        displays
            .into_iter()
            .skip(1)
            .flat_map(|display| ["-i".to_owned(), format!("{}:{}", display, background)]),
    );
    log::info!("locking screen");
    Command::new("swaylock")
        .args([
            "--ignore-empty-password",
            "--show-failed-attempts",
            "--daemonize",
            "-s",
            "center",
        ])
        .args(all_monitors)
        .spawn()?
        .wait()?;
    Ok(())
}
