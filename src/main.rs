use std::fs::{File, OpenOptions};
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use anyhow::{anyhow, Context};
use clap::Parser;
use jiff::Zoned;
use rss::{Channel, Enclosure, Guid, Item};
use url::Url;

#[derive(Debug, Parser)]
/// Download audio files from a podcast RSS feed.
struct Args {
    /// URL of the podcast RSS feed.
    url: String,

    #[arg(short, long, default_value = ".")]
    /// Audio file output directory.
    outdir: PathBuf,

    #[arg(short = 'r', long, default_value = "false")]
    /// Use the remote filename for output files instead of the date and episode title.
    use_remote_filename: bool,

    #[arg(short, long, default_value = "4")]
    /// Number of threads to use to download episodes in parallel.
    n_threads: usize,
}

/// A podcast episode
///
/// This corresponds to a single `<item>` in the podcast RSS feed.
#[derive(Debug)]
struct Episode {
    /// Podcast episode title
    title: String,
    /// Enclosure audio file URL
    audio_url: Url,
    /// Size of the audio file in bytes
    size: u64,
    /// Episode publication date
    date: Zoned,
}

impl TryFrom<Item> for Episode {
    type Error = anyhow::Error;

    fn try_from(item: Item) -> Result<Self, Self::Error> {
        let title = item
            .title()
            .or_else(|| item.guid().map(Guid::value))
            .map(sanitize_filename::sanitize)
            .context("Failed to extract item title and GUID.")?;
        let audio_url: Url = item
            .enclosure()
            .map(Enclosure::url)
            .context("Missing enclosure")?
            .parse()?;
        let size: u64 = item.enclosure().map(Enclosure::length).unwrap().parse()?;
        let date = item
            .pub_date()
            .and_then(|pd| jiff::fmt::rfc2822::parse(pd).ok())
            .context("Failed to extract item pub date.")?;
        Ok(Self {
            title,
            audio_url,
            size,
            date,
        })
    }
}

impl Episode {
    fn existing_filename(&self) -> String {
        self.audio_url
            .path_segments()
            .expect("Audio URL has no path")
            .next_back()
            .map(sanitize_filename::sanitize)
            .unwrap()
    }

    fn filename_with_date_and_title(&self) -> String {
        // eg "2025-10-19 - Podcast episode title.mp3"
        format!(
            "{} - {}.mp3",
            self.date.strftime("%F"),
            sanitize_filename::sanitize(self.title.as_str())
        )
    }
}

fn main() -> anyhow::Result<()> {
    let Args {
        url,
        outdir,
        use_remote_filename,
        n_threads,
    } = Args::parse();
    if !outdir.is_dir() {
        return Err(anyhow!("--outdir must be a directory"));
    }
    let outdir = outdir.as_path();

    let rss_content = ureq::get(&url).call()?.into_reader();
    let channel = Channel::read_from(BufReader::new(rss_content))?;

    let episodes: Vec<Episode> = channel
        .items
        .into_iter()
        .filter_map(|i| {
            Episode::try_from(i)
                .inspect_err(|e| eprintln!("{:?}", e))
                .ok()
        })
        .collect();
    let episodes: Mutex<Vec<Episode>> = Mutex::new(episodes);

    std::thread::scope(|scope| {
        for _ in 0..n_threads {
            scope.spawn(|| loop {
                let Some(episode) = episodes.lock().unwrap().pop() else {
                    break;
                };
                eprintln!("Downloading {}", episode.audio_url);
                let _ = download(episode, outdir, use_remote_filename)
                    .inspect_err(|e| eprintln!("{e}"));
            });
        }
    });

    Ok(())
}

fn download(episode: Episode, outdir: &Path, use_remote_filename: bool) -> anyhow::Result<()> {
    let filename = if use_remote_filename {
        episode.existing_filename()
    } else {
        episode.filename_with_date_and_title()
    };
    let output_file = outdir.join(filename);
    let Ok(mut file) = open_output_file(&output_file) else {
        eprintln!(
            "Skipping: Already exists: {:?}",
            output_file.to_string_lossy()
        );
        return Ok(());
    };

    let response = ureq::get(episode.audio_url.as_str()).call()?;
    let content_length: u64 = response.header("content-length").unwrap().parse()?;
    if content_length != episode.size {
        eprintln!(
            "Warning :: Expected {} bytes :: Got {} bytes ({} different)",
            episode.size,
            content_length,
            episode.size - content_length
        );
    }
    let mut response_content = response.into_reader();
    let bytes_written = std::io::copy(&mut response_content, &mut file)?;
    //eprintln!(
    //    "Wrote {} to {}",
    //    episode.audio_url,
    //    output_file.to_string_lossy()
    //);
    if bytes_written != episode.size {
        eprintln!(
            "Warning :: Expected {} bytes :: Wrote {} bytes ({} different)",
            episode.size,
            bytes_written,
            episode.size - content_length
        );
    }
    Ok(())
}

fn open_output_file(output_file: &PathBuf) -> anyhow::Result<File> {
    OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(output_file)
        .map_err(anyhow::Error::new)
}
