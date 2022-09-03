use clap::Parser;
use select::{document::Document, predicate::Name};
use std::{io, path::PathBuf, sync::Arc};
use tokio_stream::StreamExt;
use tracing::Level;
use tracing_subscriber::{util::SubscriberInitExt, EnvFilter, FmtSubscriber};

#[derive(Parser, Clone, Debug)]
#[clap(author, version)]
struct Args {
    url: reqwest::Url,
    #[clap(parse(from_os_str), default_value = "out")]
    out_dir: PathBuf,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    setup_tracing()?;
    let args = Args::parse();
    let document = {
        let res = reqwest::get(args.url.clone()).await?.text().await?;
        Document::from(res.as_str())
    };
    tokio::fs::create_dir_all(&args.out_dir).await?;
    let mut receiver = download_images(
        Arc::new(args.out_dir.clone()),
        document
            .find(Name("img"))
            .filter_map(|node| node.attr("src"))
            .filter_map(|image_source| image_url(&args.url, image_source)),
    );
    while let Some(result) = receiver.recv().await {
        match result {
            Ok(path) => println!("{}", path),
            Err(error) => eprintln!("{}", error),
        }
    }
    Ok(())
}

#[tracing::instrument(skip(images_urls))]
fn download_images<I>(
    out_dir: Arc<PathBuf>,
    images_urls: I,
) -> tokio::sync::mpsc::Receiver<anyhow::Result<String>>
where
    I: IntoIterator<Item = reqwest::Url>,
{
    tracing::debug!("downloading images");
    let backpressure = std::thread::available_parallelism().unwrap().get();
    let (sender, receiver) = tokio::sync::mpsc::channel(backpressure * 2);
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

#[tracing::instrument(skip(out_dir))]
async fn download_image(out_dir: Arc<PathBuf>, url: reqwest::Url) -> anyhow::Result<String> {
    let image = reqwest::get(url.clone())
        .await?
        .bytes_stream()
        .map(to_io_error);
    let file = url.path_segments().unwrap().last().unwrap();
    let decoded = urlencoding::decode(file)?;
    let file = out_dir.join(decoded.as_ref());
    let mut writer = tokio::fs::File::create(&file).await?;
    let mut reader = tokio_util::io::StreamReader::new(image);
    tokio::io::copy(&mut reader, &mut writer).await?;
    tracing::debug!("image {:?} written", file);
    Ok(decoded.into_owned())
}

fn image_url(base_url: &reqwest::Url, image_source: &str) -> Option<reqwest::Url> {
    if let Ok(url) = reqwest::Url::parse(image_source) {
        Some(url)
    } else {
        base_url.join(image_source).ok()
    }
}

fn setup_tracing() -> anyhow::Result<()> {
    Ok(FmtSubscriber::builder()
        .with_max_level(Level::DEBUG)
        .with_env_filter(
            EnvFilter::builder()
                .with_default_directive(concat!(env!("CARGO_PKG_NAME"), "=info").parse()?)
                .from_env()?,
        )
        .pretty()
        .finish()
        .try_init()?)
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
