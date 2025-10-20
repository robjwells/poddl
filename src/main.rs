use std::fs::{File, OpenOptions};
use std::io::{BufReader, Read};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use anyhow::{anyhow, Context};
use clap::{Args, Parser};
use jiff::Zoned;
use rss::{Channel, Enclosure, Guid, Item};
use url::Url;

#[derive(Debug, Parser)]
/// Download audio files from a podcast RSS feed.
struct CliArgs {
    /// Url of RSS feed or saved XML file input.
    #[command(flatten)]
    input: InputArgs,

    #[arg(short, long = "output-dir", default_value = ".")]
    /// Audio file output directory.
    output_directory: PathBuf,

    #[arg(short = 'r', long, default_value = "false")]
    /// Use the remote filename for output files instead of the date and episode title.
    use_remote_filename: bool,

    #[arg(short, long, default_value = "4")]
    /// Number of threads to use to download episodes in parallel.
    n_threads: usize,
}

#[derive(Debug, Args)]
#[group(required = true, multiple = false)]
struct InputArgs {
    /// URL of the podcast RSS feed.
    url: Option<String>,

    /// File containing RSS feed.
    #[arg(short, long)]
    file: Option<PathBuf>,
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

impl TryFrom<&Item> for Episode {
    type Error = anyhow::Error;

    fn try_from(item: &Item) -> Result<Self, Self::Error> {
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

fn load_rss_channel(url: Option<String>, file: Option<PathBuf>) -> anyhow::Result<Channel> {
    let reader: Box<dyn Read> = if let Some(url) = url {
        let response = ureq::get(&url).call()?;
        Box::new(response.into_body().into_reader())
    } else if let Some(file) = file {
        let file = std::fs::OpenOptions::new().read(true).open(&file)?;
        Box::new(file)
    } else {
        unreachable!("Clap should ensure either URL or file is provided.");
    };
    let channel = Channel::read_from(BufReader::new(reader))?;
    Ok(channel)
}

fn extract_episodes(channel: &Channel) -> Vec<Episode> {
    channel
        .items
        .iter()
        .filter_map(|i| {
            Episode::try_from(i)
                .inspect_err(|e| log::error!("{:?}", e))
                .ok()
        })
        .collect()
}

fn main() -> anyhow::Result<()> {
    env_logger::init();

    let args = CliArgs::parse();
    log::debug!("{args:#?}");
    let CliArgs {
        input: InputArgs { url, file },
        output_directory,
        use_remote_filename,
        n_threads,
    } = args;

    if !output_directory.is_dir() {
        return Err(anyhow!("output-dir must be a directory"));
    }

    let channel = load_rss_channel(url, file)?;
    let episodes = extract_episodes(&channel);
    log::info!("{} episodes in RSS feed", episodes.len());
    let episodes: Mutex<Vec<Episode>> = Mutex::new(episodes);

    std::thread::scope(|scope| {
        for _ in 0..n_threads {
            scope.spawn(|| loop {
                let Some(episode) = episodes.lock().unwrap().pop() else {
                    break;
                };
                log::info!("Downloading {:?}", episode.filename_with_date_and_title());
                log::debug!("{}", episode.audio_url);
                // Download file, log but continue on error.
                let _ = download(episode, &output_directory, use_remote_filename)
                    .inspect_err(|e| log::error!("{e}"));
            });
        }
    });

    Ok(())
}

fn download(
    episode: Episode,
    output_directory: &Path,
    use_remote_filename: bool,
) -> anyhow::Result<()> {
    let filename = if use_remote_filename {
        episode.existing_filename()
    } else {
        episode.filename_with_date_and_title()
    };
    let output_file = output_directory.join(filename);
    let Ok(mut file) = open_output_file(&output_file) else {
        log::info!(
            "Skipping as file already exists: {:?}",
            output_file.to_string_lossy()
        );
        return Ok(());
    };

    let response = ureq::get(episode.audio_url.as_str()).call()?;
    let content_length: u64 = response
        .headers()
        .get("content-length")
        .unwrap()
        .to_str()?
        .parse()?;
    // Report if the header indicates a different size to the enclosure.
    if content_length != episode.size {
        let content_kib = content_length / 1024;
        let size_kib = episode.size / 1024;
        // TODO: This can report a difference of 0 when diff < 1024 bytes.
        log::warn!(
            "Size mismatch :: Enclosure: {} KiB :: Header: {} ({} different)",
            size_kib,
            content_kib,
            size_kib.abs_diff(content_kib)
        );
    }
    let mut response_content = response.into_body().into_reader();
    let bytes_written = std::io::copy(&mut response_content, &mut file)?;
    // Report if we wrote a different number of bytes than the header indicated.
    if bytes_written != content_length {
        let written_kib = bytes_written / 1024;
        let size_kib = episode.size / 1024;
        // TODO: This can report a difference of 0 when diff < 1024 bytes.
        log::warn!(
            "Size mismatch :: Header: {} KiB :: Written: {} ({} different)",
            size_kib,
            written_kib,
            size_kib.abs_diff(written_kib)
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
