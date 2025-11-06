## poddl

`poddl` is a command-line tool to download podcast episodes from an RSS feed.

Provide it with the URL of a podcast feed (or a local path with `--file`), and
it will save each episode to the output directory (`-o`, current directory by
default).

It defaults to naming each episode after its date and title, but you can choose
to use the filename present in the RSS feed (with `-r`), though note that the
filenames in the RSS feed depend entirely on the podcast, so they may not be
numbered or ordered, or may be complete inscrutable (such as with BBC
podcasts).

`poddl` uses two threads by default to download episodes concurrently. You can
increase this to potentially download all episodes more quickly, with
correspondingly higher load on the remote server.

Log output can be suppressed (or increased) by setting the [RUST_LOG][] environment
variable. Use `RUST_LOG=off` to suppress all output.

[RUST_LOG]: https://docs.rs/env_logger/latest/env_logger/#enabling-logging

### Example usage

Let's download the public feed for [Fourth Reich Archaeology][4ra], a great
podcast that examines the roots of our current political system. The podcast
page is [on Spotify][4ra-spotify], from which you can find the [RSS feed
link][4ra-rss] (the orange icon with a dot in the corner and "radio waves"
emanating from it).

```sh
mkdir 4ra
poddl https://anchor.fm/s/f94a82ac/podcast/rss \
    --output-directory 4ra \
    --keep-rss-feed
```

And the output:

```
[2025-11-06T10:59:47Z INFO  poddl] 70 episodes in RSS feed
[2025-11-06T10:59:47Z INFO  poddl] Downloading 2024-08-02 "#001 - Fourth Reich Archaeology An Introduction" to "4ra/2024-08-02 - #001 - Fourth Reich Archaeology An Introduction.m4a"
[2025-11-06T10:59:47Z INFO  poddl] Downloading 2024-07-31 "Intro Music" to "4ra/2024-07-31 - Intro Music.m4a"
[2025-11-06T10:59:47Z INFO  poddl] Wrote RSS feed to "4ra/2025-11-06 - Fourth Reich Archaeology.rss"
[2025-11-06T10:59:47Z INFO  poddl] Downloading 2024-08-23 "#004 - Jerryworld 3 Boola Boola" to "4ra/2024-08-23 - #004 - Jerryworld 3 Boola Boola.mp3"
...
```

[4ra]: https://creators.spotify.com/pod/profile/fourth-reich-archaeology/
[4ra-spotify]: https://creators.spotify.com/pod/profile/fourth-reich-archaeology/
[4ra-rss]: https://anchor.fm/s/f94a82ac/podcast/rss

### CLI help message

```
poddl: Download audio files from a podcast RSS feed

Provide the URL or file path (with --file) of an RSS feed, and poddl will
download each episode. Files will be saved in the current directory by default,
use the -o option to choose another directory.

Episodes will be saved to files named with the episode date and title, use the
-r|--use-remote-filename option to use the episode filename that appears in the
RSS feed enclosure tag instead.

The podcast feed can be written to the output directory with the
-k|--keep-rss-feed option.

Two episodes are downloaded at a time in separate threads, use the
-n|--n-threads option to change this.

Usage: poddl [OPTIONS] <URL|--file <FILE>>

Arguments:
  [URL]
          URL of the podcast RSS feed

Options:
  -f, --file <FILE>
          File containing RSS feed

  -o, --output-dir <OUTPUT_DIRECTORY>
          Output directory
          
          [default: .]

  -r, --use-remote-filename
          Use the RSS filename for output files instead of the date and episode title

  -k, --keep-rss-feed
          Save the RSS feed to the output directory

  -n, --n-threads <N_THREADS>
          Number of threads to use to download episodes concurrently
          
          [default: 2]

  -h, --help
          Print help (see a summary with '-h')
```
