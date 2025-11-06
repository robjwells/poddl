use std::path::PathBuf;

use clap::{Args, Parser};

/// poddl: Download audio files from a podcast RSS feed
///
/// Provide the URL or file path (with --file) of an RSS feed, and poddl will download
/// each episode. Files will be saved in the current directory by default, use the
/// -o option to choose another directory.
///
/// Episodes will be saved to files named with the episode date and title, use the
/// -r|--use-remote-filename option to use the episode filename that appears in the
/// RSS feed enclosure tag instead.
///
/// The podcast feed can be written to the output directory with the
/// -k|--keep-rss-feed option.
///
/// Two episodes are downloaded at a time in separate threads, use the -n|--n-threads
/// option to change this.
#[derive(Debug, Parser)]
pub(crate) struct CliArgs {
    /// URL of RSS feed or path to saved XML file.
    #[command(flatten)]
    pub input: InputArgs,

    /// Output directory.
    #[arg(short, long = "output-dir", default_value = ".")]
    pub output_directory: PathBuf,

    /// Use the RSS filename for output files instead of the date and episode title.
    #[arg(short = 'r', long, default_value = "false")]
    pub use_remote_filename: bool,

    /// Save the RSS feed to the output directory.
    #[arg(short, long, default_value = "false")]
    pub keep_rss_feed: bool,

    /// Number of threads to use to download episodes concurrently.
    #[arg(short, long, default_value = "2")]
    pub n_threads: usize,
}

#[derive(Debug, Args)]
#[group(required = true, multiple = false)]
pub(crate) struct InputArgs {
    /// URL of the podcast RSS feed.
    pub url: Option<String>,

    /// File containing RSS feed.
    #[arg(short, long)]
    pub file: Option<PathBuf>,
}
