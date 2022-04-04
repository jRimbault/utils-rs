use clap::Parser;
use select::document::Document;
use select::predicate::Name;
use std::path::{Path, PathBuf};

#[derive(Parser, Clone)]
#[clap(author, version)]
struct Cli {
    url: reqwest::Url,
    #[clap(parse(from_os_str), default_value = "out")]
    out_dir: PathBuf,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let Cli { url, out_dir } = Cli::parse();
    let document = {
        let res = reqwest::get(url.clone()).await?.text().await?;
        Document::from(res.as_str())
    };
    let images_urls = document
        .find(Name("img"))
        .filter_map(|node| node.attr("src"))
        .filter_map(|image_source| image_url(&url, image_source));
    tokio::fs::create_dir_all(&out_dir).await?;

    let mut receiver = {
        let (sender, receiver) = tokio::sync::mpsc::channel(32);
        for url in images_urls {
            let sender = sender.clone();
            let out_dir = out_dir.clone();
            tokio::spawn(async move {
                sender
                    .send(download_image(&out_dir, url).await)
                    .await
                    .unwrap();
            });
        }
        receiver
    };
    while let Some(result) = receiver.recv().await {
        match result {
            Ok(path) => println!("{}", path),
            Err(error) => eprintln!("{}", error),
        }
    }
    Ok(())
}

async fn download_image(out_dir: &Path, url: reqwest::Url) -> anyhow::Result<String> {
    let image = reqwest::get(url.clone()).await?;
    let file = url.path_segments().unwrap().last().unwrap();
    let file = out_dir.join(file);
    let mut file = tokio::fs::File::create(file).await?;
    let bytes = image.bytes().await?;
    let mut bytes = bytes.as_ref();
    tokio::io::copy(&mut bytes, &mut file).await?;
    Ok(url.path().to_owned())
}

fn image_url(base_url: &reqwest::Url, image_source: &str) -> Option<reqwest::Url> {
    let r1 = reqwest::Url::parse(image_source);
    if let Ok(url) = r1 {
        return Some(url);
    }
    let r2 = base_url.join(image_source);
    if let Ok(url) = r2 {
        return Some(url);
    }
    None
}
