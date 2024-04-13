pub mod comic;

use std::{
    cmp::Reverse,
    process::{Command, Stdio},
};

use serde::Deserialize;

pub fn safe_path(value: &str) -> String {
    value
        .chars()
        .filter(|&c| c.is_alphanumeric() || c == ' ')
        .collect()
}

pub fn displays() -> anyhow::Result<Vec<String>> {
    #[derive(Debug, Deserialize)]
    struct Output {
        name: String,
        rect: Dimensions,
    }
    #[derive(Debug, Deserialize)]
    struct Dimensions {
        width: u32,
    }
    let mut child = Command::new("swaymsg")
        .args(["-t", "get_outputs"])
        .stdout(Stdio::piped())
        .spawn()?;
    let mut outputs: Vec<Output> = serde_json::from_reader(child.stdout.take().unwrap())?;
    outputs.sort_by_key(|o| Reverse(o.rect.width));
    Ok(outputs.into_iter().map(|o| o.name).collect())
}
