use std::path::PathBuf;

use clap::{Args, Parser};

#[derive(Debug, Parser)]
/// Download audio files from a podcast RSS feed.
pub(crate) struct CliArgs {
    /// Url of RSS feed or saved XML file input.
    #[command(flatten)]
    pub input: InputArgs,

    /// Audio file output directory.
    #[arg(short, long = "output-dir", default_value = ".")]
    pub output_directory: PathBuf,

    /// Use the remote filename for output files instead of the date and episode title.
    #[arg(short = 'r', long, default_value = "false")]
    pub use_remote_filename: bool,

    /// Save the RSS feed to the output directory.
    #[arg(short, long, default_value = "false")]
    pub keep_rss_feed: bool,

    /// Number of threads to use to download episodes in parallel.
    #[arg(short, long, default_value = "4")]
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
