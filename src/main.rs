use std::{env::args, path::PathBuf};

use futures::future::join_all;
use rss::{Channel, Item};
use tokio::{fs::OpenOptions, io::AsyncWriteExt};

use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    let rss_url = args().nth(1).unwrap();
    let bytes = reqwest::get(&rss_url).await?.bytes().await?;
    let channel = Channel::read_from(bytes.as_ref())?;

    let handles: Vec<_> = channel.items
        .into_iter()
        .map(download_item)
        .map(tokio::spawn)
        .collect();
    join_all(handles).await;
    Ok(())
}

async fn download_item(item: Item) -> Result<()> {
    let name = item
        .title()
        .or_else(|| item.guid().map(|g| g.value()))
        .unwrap();

    let filename = PathBuf::from(format!("{name}.mp3"));

    let maybe_file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&filename)
        .await;
    let Ok(mut file) = maybe_file else {
        println!("Skipping  {}", filename.to_string_lossy());
        return Ok(())
    };

    let url = item.enclosure().unwrap().url();
    println!("Fetching  {}", url);
    let response = reqwest::get(url).await?;
    file.write_all(response.bytes().await?.as_ref()).await?;
    println!("Wrote     {}", filename.to_string_lossy());
    Ok(())
}
