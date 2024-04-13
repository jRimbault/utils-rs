use std::{
    fs::File,
    path::{Path, PathBuf},
    process::Command,
    time::{Duration, SystemTime},
};

use rand::Rng;
use serde::Deserialize;

pub fn dir() -> PathBuf {
    dirs::picture_dir()
        .expect("you should have a Pictures directory")
        .join("xkcd")
}

#[derive(Debug, Default, Deserialize)]
pub struct Xkcd {
    img: String,
    title: String,
    alt: String,
    num: u32,
}

impl Xkcd {
    pub fn random() -> anyhow::Result<Xkcd> {
        let num = Xkcd::latest_n()?;
        let n = rand::thread_rng().gen_range(1..=num);
        Xkcd::number(n)
    }

    /// I use a file ~/Pictures/xkcd/latest/keep to keep the number of
    /// the latest comic. The last time this file was modified acts as
    /// a marker to trigger an update. Otherwise I'll just read the number
    /// from the file directly.
    fn latest_n() -> anyhow::Result<u32> {
        let dir = dir().join("latest");
        std::fs::create_dir_all(&dir)?;
        let cache = dir.join("keep");
        let last_modified = cache
            .metadata()
            .and_then(|m| m.modified())
            .unwrap_or(SystemTime::UNIX_EPOCH);
        let since_last_modified = SystemTime::now().duration_since(last_modified)?;
        let one_day = Duration::from_secs(24 * 3600);
        if since_last_modified > one_day {
            log::info!("updating latest comic");
            let latest = Xkcd::latest()?;
            std::fs::write(cache, latest.num.to_le_bytes())?;
            return Ok(latest.num);
        }
        log::info!("reusing latest comic");
        let bytes = std::fs::read(cache)?;
        Ok(u32::from_le_bytes(bytes[..4].try_into()?))
    }

    fn latest() -> anyhow::Result<Xkcd> {
        log::debug!("getting latest xkcd");
        Ok(ureq::get("https://xkcd.com/info.0.json")
            .call()?
            .into_json()?)
    }

    fn number(n: u32) -> anyhow::Result<Xkcd> {
        if let Some(comic) = Xkcd::search_for(n) {
            log::info!("found comic #{n} in picture cache");
            return Ok(comic);
        }
        log::info!("getting comic #{n} infos");
        Ok(ureq::get(&format!("https://xkcd.com/{n}/info.0.json"))
            .call()?
            .into_json()?)
    }

    fn search_for(n: u32) -> Option<Xkcd> {
        // I should have split my picture cache into buckets from the start
        // rookie mistake
        for entry in dir().read_dir().unwrap().flatten() {
            if entry.metadata().unwrap().is_dir() {
                continue;
            }
            let name = entry.file_name();
            let name = name.to_str().unwrap();
            let i: u32 = name[0..4]
                .parse()
                .expect("there should only be comics in this directory");
            if i == n {
                return Some(Xkcd {
                    num: n,
                    title: name[7..name.len() - 4].to_string(),
                    ..Default::default()
                });
            }
        }
        log::info!("comic #{} not found in cache", n);
        None
    }

    pub fn download(&self) -> anyhow::Result<PathBuf> {
        let xkcd_dir = dir();
        std::fs::create_dir_all(&xkcd_dir)?;
        let xkcd = xkcd_dir.join(self.filename());
        if xkcd.exists() {
            log::debug!("using cache of comic #{}", self.num);
            Ok(xkcd)
        } else {
            log::info!("downloading comic #{} to cache", self.num);
            let mut reader = ureq::get(&self.img).call()?.into_reader();
            std::io::copy(&mut reader, &mut File::create(&xkcd)?)?;
            Ok(xkcd)
        }
    }

    /// Bad API
    pub fn write_to_file_as_bg(&self, file: &Path) -> anyhow::Result<PathBuf> {
        let xkcd_dir = dir().join("with_text");
        std::fs::create_dir_all(&xkcd_dir)?;
        let xkcd_bg = xkcd_dir.join(self.filename());
        if xkcd_bg.exists() {
            log::info!("using cache of background comic #{}", self.num);
            Ok(xkcd_bg)
        } else {
            log::info!(
                "writting background version of comic #{} to cache",
                self.num
            );
            let alt = textwrap::wrap(&self.alt, 70).join("\n");
            Command::new("convert")
                .args(["-size", "1920x1080", "xc:white"])
                .arg(file)
                .args([
                    "-gravity",
                    "center",
                    "-gravity",
                    "center",
                    "-composite",
                    "-gravity",
                    "north",
                    "-pointsize",
                    "36",
                    "-annotate",
                    "+0+100",
                ])
                .arg(&self.title)
                .args([
                    "-gravity",
                    "south",
                    "-pointsize",
                    "20",
                    "-annotate",
                    "+0+100",
                ])
                .arg(alt)
                .arg(&xkcd_bg)
                .spawn()?
                .wait()?;
            Ok(xkcd_bg)
        }
    }

    fn filename(&self) -> String {
        format!("{:0>4} - {}.png", self.num, crate::safe_path(&self.title))
    }
}
