use clap::Parser;
use select::document::Document;
use select::predicate::Name;
use tokio_stream::{self as stream, StreamExt};

#[derive(Parser)]
#[clap(author, version)]
struct Cli {
    url: reqwest::Url,
    #[clap(parse(from_os_str), default_value = "out")]
    out_dir: std::path::PathBuf,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Cli::parse();
    let document = {
        let res = reqwest::get(args.url.clone()).await?.text().await?;
        Document::from(res.as_str())
    };
    let images_urls = document
        .find(Name("img"))
        .filter_map(|node| node.attr("src"))
        .filter_map(|image_source| image_url(&args.url, image_source).ok());
    tokio::fs::create_dir_all(&args.out_dir).await?;
    let mut iter = stream::iter(images_urls).map(|url| async { download_image(&args, url).await });
    while let Some(r) = iter.next().await {
        println!("{}", r.await?);
    }
    Ok(())
}

async fn download_image(args: &Cli, url: reqwest::Url) -> anyhow::Result<String> {
    let image = reqwest::get(url.clone()).await?;
    let file = url.path_segments().unwrap().last().unwrap();
    let file = args.out_dir.join(file);
    let mut file = tokio::fs::File::create(file).await?;
    let bytes = image.bytes().await?;
    let mut bytes = bytes.as_ref();
    tokio::io::copy(&mut bytes, &mut file).await?;
    Ok(url.path().to_owned())
}

fn image_url(base_url: &reqwest::Url, image_source: &str) -> anyhow::Result<reqwest::Url> {
    let r1 = reqwest::Url::parse(image_source);
    if let Ok(url) = r1 {
        return Ok(url);
    }
    let r2 = base_url.join(image_source);
    if let Ok(url) = r2 {
        return Ok(url);
    }
    anyhow::bail!("{} and {}", r1.unwrap_err(), r2.unwrap_err())
}
