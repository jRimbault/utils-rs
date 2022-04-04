use clap::Parser;
use select::document::Document;
use select::predicate::Name;
use std::io;
use std::path::PathBuf;
use std::sync::Arc;
use tokio_stream::StreamExt;

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
    tokio::fs::create_dir_all(&out_dir).await?;
    let mut receiver = download_images(
        Arc::new(out_dir),
        document
            .find(Name("img"))
            .filter_map(|node| node.attr("src"))
            .filter_map(|image_source| image_url(&url, image_source)),
    );
    while let Some(result) = receiver.recv().await {
        match result {
            Ok(path) => println!("{}", path),
            Err(error) => eprintln!("{}", error),
        }
    }
    Ok(())
}

fn download_images<I>(
    out_dir: Arc<PathBuf>,
    images_urls: I,
) -> tokio::sync::mpsc::Receiver<anyhow::Result<String>>
where
    I: IntoIterator<Item = reqwest::Url>,
{
    let backpressure = std::thread::available_parallelism().unwrap().get();
    let (sender, receiver) = tokio::sync::mpsc::channel(backpressure);
    for url in images_urls {
        let sender = sender.clone();
        let out_dir = Arc::clone(&out_dir);
        tokio::spawn(async move {
            sender
                .send(download_image(out_dir, url).await)
                .await
                .unwrap();
        });
    }
    receiver
}

async fn download_image(out_dir: Arc<PathBuf>, url: reqwest::Url) -> anyhow::Result<String> {
    let image = reqwest::get(url.clone())
        .await?
        .bytes_stream()
        .map(to_io_error);
    let file = url.path_segments().unwrap().last().unwrap();
    let decoded = urlencoding::decode(file)?;
    let file = out_dir.join(decoded.as_ref());
    let mut writer = tokio::fs::File::create(file).await?;
    let mut reader = tokio_util::io::StreamReader::new(image);
    tokio::io::copy(&mut reader, &mut writer).await?;
    Ok(decoded.into_owned())
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

fn to_io_error<T, E>(result: Result<T, E>) -> io::Result<T>
where
    E: Into<Box<dyn std::error::Error + Send + Sync>>,
{
    match result {
        Ok(value) => Ok(value),
        Err(error) => Err(io::Error::new(io::ErrorKind::Other, error)),
    }
}
