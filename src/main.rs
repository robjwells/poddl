use std::fs::{File, OpenOptions};
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use anyhow::{anyhow, Context};
use clap::Parser;
use jiff::Zoned;
use rss::{Channel, Guid, Item};
use url::Url;

use crate::cli::InputArgs;

mod cli;

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
    #[allow(dead_code)]
    size: u64,
    /// Episode publication date
    date: Zoned,
    /// Enclosure mime type, indicates the extension.
    mime_type: String,
}

impl TryFrom<&Item> for Episode {
    type Error = anyhow::Error;

    fn try_from(item: &Item) -> Result<Self, Self::Error> {
        let title = item
            .title()
            .or_else(|| item.guid().map(Guid::value))
            .map(sanitize_filename::sanitize)
            .context("Failed to extract item title and GUID.")?;
        let enclosure = item.enclosure().context("Missing enclosure")?;
        let audio_url: Url = enclosure.url().parse()?;
        let size: u64 = enclosure.length.parse()?;
        let mime_type = enclosure.mime_type.clone();
        let date = item
            .pub_date()
            .and_then(|pd| jiff::fmt::rfc2822::parse(pd).ok())
            .context("Failed to extract item pub date.")?;
        Ok(Self {
            title,
            audio_url,
            size,
            date,
            mime_type,
        })
    }
}

impl Episode {
    /// Produce an extension matching the mime type
    ///
    /// [Apple lists] the following supported file formats:
    ///
    /// - M4A: audio/x-m4a
    /// - MP3: audio/mpeg
    /// - MOV: video/quicktime
    /// - MP4: video/mp4
    /// - M4V: video/x-m4v
    /// - PDF: application/pdf
    ///
    /// [Apple lists]: https://help.apple.com/itc/podcasts_connect/#/itcb54353390
    fn extension(&self) -> &'static str {
        match self.mime_type.as_ref() {
            "audio/mpeg" => "mp3",
            "audio/x-m4a" => "m4a",
            "video/quicktime" => "mov",
            "video/mp4" => "mp4",
            "video/x-m4v" => "m4v",
            "application/pdf" => "pdf",
            mt => panic!("unexpected mime time {:?}", mt),
        }
    }

    /// Filename from the RSS feed enclosure.
    fn existing_filename(&self) -> String {
        self.audio_url
            .path_segments()
            .expect("Audio URL has no path")
            .next_back()
            .map(sanitize_filename::sanitize)
            .unwrap()
    }

    /// Filename composed of the title prefixed with the episode's ISO date.
    fn filename_with_date_and_title(&self) -> String {
        // Limit the length of the filename part used by the title.
        let title = {
            // 255 is the truncate limit for sanitize_filename, -10 for the date, -3 for the
            // separator, -4 for the extension.
            let mut limit: usize = 255 - 10 - 3 - 4;
            while !self.title.is_char_boundary(limit) {
                limit -= 1;
            }
            self.title.split_at(limit).0
        };

        format!(
            "{} - {}.{}",
            self.date.strftime("%F"),
            sanitize_filename::sanitize(title),
            self.extension()
        )
    }

    /// Select a filename based on the user's choice.
    fn filename(&self, use_remote_filename: bool) -> String {
        if use_remote_filename {
            self.existing_filename()
        } else {
            self.filename_with_date_and_title()
        }
    }
}

/// Read RSS feed bytes from a URL or a file.
fn load_rss_bytes(input: &InputArgs) -> anyhow::Result<Vec<u8>> {
    let InputArgs { url, file } = input;

    let bytes = if let Some(url) = url {
        let response = ureq::get(url).call()?;
        response.into_body().read_to_vec()?
    } else if let Some(file) = file {
        std::fs::read(file)?
    } else {
        unreachable!("Clap should ensure either URL or file is provided.");
    };

    Ok(bytes)
}

/// Extract episode information from the RSS feed.
fn extract_episodes(channel: &Channel) -> Vec<Episode> {
    let episodes: Vec<Episode> = channel
        .items
        .iter()
        .filter_map(|i| {
            Episode::try_from(i)
                .inspect_err(|e| log::error!("{:?}", e))
                .ok()
        })
        .collect();
    log::info!("{} episodes in RSS feed", episodes.len());
    episodes
}

/// Wrapper around CliArgs::parse that logs the received struct.
fn parse_args() -> cli::CliArgs {
    let args = cli::CliArgs::parse();
    log::debug!("{args:#?}");
    args
}

/// Enable info-level logging for the binary by default.
fn enable_info_logs() {
    use env_logger::{Builder, Env};
    Builder::from_env(Env::default().default_filter_or("poddl=info")).init();
}

/// Make sure the chosen output directory exists as a directory.
///
/// Creates the directory if it does not already exist.
fn ensure_output_directory(output_directory: &Path) -> anyhow::Result<()> {
    // Something else is already present at output_directory.
    if output_directory.exists() && !output_directory.is_dir() {
        return Err(anyhow!(
            "Chosen output directory exists and is not a directory."
        ));
    }
    // Create the directory if it does not exist.
    if !output_directory.exists() {
        std::fs::create_dir(output_directory)?;
    }
    Ok(())
}

/// Write a date-prefixed RSS feed to the output directory.
///
/// Logs but otherwise ignores any error.
fn write_rss_feed(channel_title: &str, output_directory: &Path, rss_bytes: &[u8]) {
    // Eg "2025-10-21 - In Our Time.rss"
    let filename = sanitize_filename::sanitize(format!(
        "{} - {}.rss",
        jiff::Zoned::now().strftime("%F"),
        channel_title
    ));
    let path = output_directory.join(filename);
    match std::fs::write(&path, rss_bytes) {
        Ok(()) => log::info!("Wrote RSS feed to {:?}", path.to_string_lossy()),
        Err(e) => log::error!("Failed to write RSS feed to output directory: {e}"),
    };
}

fn main() -> anyhow::Result<()> {
    enable_info_logs();
    let args = parse_args();

    let output_directory = args.output_directory.as_path();
    ensure_output_directory(output_directory)?;

    let bytes = load_rss_bytes(&args.input)?;
    let channel = Channel::read_from(Cursor::new(&bytes))?;
    let episodes = Mutex::new(extract_episodes(&channel));

    std::thread::scope(|scope| {
        if args.keep_rss_feed {
            scope.spawn(|| write_rss_feed(channel.title(), output_directory, &bytes));
        }
        // Create n_threads downloader threads.
        for _ in 0..args.n_threads {
            scope.spawn(|| loop {
                let Some(episode) = episodes.lock().unwrap().pop() else {
                    break;
                };
                // Download file, log but continue on error.
                let _ = download(episode, output_directory, args.use_remote_filename)
                    .inspect_err(|e| log::error!("{e}"));
            });
        }
    });

    Ok(())
}

/// Download an episode to a file.
fn download(
    episode: Episode,
    output_directory: &Path,
    use_remote_filename: bool,
) -> anyhow::Result<()> {
    let output_file = output_directory.join(episode.filename(use_remote_filename));
    log::info!(
        "Downloading {} {:?} to {:?}",
        episode.date.strftime("%F"),
        episode.title,
        output_file.to_string_lossy(),
    );
    let Ok(mut file) = open_output_file(&output_file) else {
        log::info!(
            "Skipping as file already exists: {:?}",
            output_file.to_string_lossy()
        );
        return Ok(());
    };

    log::debug!("{}", episode.audio_url);
    let response = ureq::get(episode.audio_url.as_str()).call()?;
    let mut response_content = response.into_body().into_reader();
    let _ = std::io::copy(&mut response_content, &mut file)?;

    Ok(())
}

/// Open a new file for writing at the given path.
fn open_output_file(output_file: &PathBuf) -> anyhow::Result<File> {
    OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(output_file)
        .map_err(anyhow::Error::new)
}
