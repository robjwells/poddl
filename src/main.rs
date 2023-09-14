use std::path::{self, PathBuf};

use anyhow::{anyhow, Result};
use clap::Parser;
use futures::future::join_all;
use rss::{Channel, Guid, Item};
use tokio::{fs::OpenOptions, io::AsyncWriteExt};

#[derive(Debug, Parser)]
/// Download audio files from a podcast RSS feed.
struct Args {
    /// URL of the podcast RSS feed.
    url: String,

    #[arg(short, long, default_value = ".")]
    /// Audio file output directory.
    outdir: PathBuf,
}

#[tokio::main]
async fn main() -> Result<()> {
    let Args { url, outdir } = Args::parse();
    if !outdir.is_dir() {
        return Err(anyhow!("--outdir must be a directory"));
    }

    let bytes = reqwest::get(&url).await?.bytes().await?;
    let channel = Channel::read_from(bytes.as_ref())?;

    let handles: Vec<_> = channel
        .items
        .into_iter()
        .map(|item| download_item(item, outdir.clone()))
        .map(tokio::spawn)
        .collect();
    join_all(handles).await;
    Ok(())
}

async fn download_item(item: Item, outdir: PathBuf) -> Result<()> {
    let name = item
        .title()
        .or_else(|| item.guid().map(Guid::value))
        .expect("Failed to extract item title and GUID.")
        .replace(path::is_separator, "-")
        + ".mp3";

    let output_file = outdir.join(name);
    let maybe_file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&output_file)
        .await;
    let Ok(mut file) = maybe_file else {
        eprintln!("Skipping  {}", output_file.to_string_lossy());
        return Ok(());
    };

    let url = item.enclosure().unwrap().url();

    eprintln!("Fetching  {}", url);
    let response = reqwest::get(url).await?;

    file.write_all(response.bytes().await?.as_ref()).await?;
    eprintln!("Wrote     {}", output_file.to_string_lossy());
    Ok(())
}
