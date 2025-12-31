use clap::builder::TypedValueParser;
use clap::{crate_authors, crate_version, value_parser, ColorChoice, Parser, Subcommand};
use clap_complete::Shell;
use music_organizer::FileOpType;
use std::path::{Path, PathBuf};

#[derive(Parser)]
#[clap(
    name = "music-organizer",
    version = crate_version!(),
    author = crate_authors!(),
    color = ColorChoice::Auto,
    about = "",
)]
pub struct Args {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
#[command()]
pub enum Command {
    /// Moves/copies, renames and retags Music files using their metadata.
    Organize(OrganizeCommand),

    /// Transcodes FLAC songs to OPUS.
    Transcode(TranscodeCommand),

    /// Generates a completion script for the specified shell.
    Completions(CompletionsCommand),
}

#[derive(Parser)]
pub struct OrganizeCommand {
    /// The directory which will be searched for music files.
    #[clap(
        long = "music-dir",
        short = 'm',
        value_parser = music_dir_value_parser(),
        default_value = "~/Music",
    )]
    pub music_dir: PathBuf,

    /// The directory which the content will be written to.
    #[clap(
        long = "output-dir",
        short = 'o',
        value_parser = output_dir_value_parser(),
    )]
    pub output_dir: Option<PathBuf>,

    /// Verbosity level of the output. 0 means least 2 means most verbose ouput.
    #[clap(
        long = "verbosity",
        short = 'v',
        value_name = "level",
        value_parser = value_parser!(u8).range(0..=2),
        default_value = "1",
    )]
    pub verbosity: u8,

    /// Copy the files instead of moving.
    #[clap(long = "copy", short = 'c', requires = "output-dir")]
    pub copy: bool,

    /// Assumes yes as a answer for questions.
    #[clap(long = "assume-yes", short = 'y')]
    pub assume_yes: bool,

    /// Only check files and print actions don't change anything.
    #[clap(long = "dry-run", short = 'd', conflicts_with = "assume_yes")]
    pub dry_run: bool,

    /// Don't check for inconsistencies.
    #[clap(long = "nocheck", short = 'n')]
    pub no_check: bool,

    /// What to do with embedded artworks
    #[clap(long = "embedded-artworks", short = 'e', default_value = "remove-redundant")]
    pub embedded_artworks: EmbeddedArtworks,

    /// Don't remove empty directories.
    #[clap(long = "nocleanup")]
    pub no_cleanup: bool,

    /// Prints timing information
    #[clap(long = "timings", short = 't')]
    pub timings: bool,
}

impl OrganizeCommand {
    pub fn op_type(&self) -> FileOpType {
        match self.copy {
            true => FileOpType::Copy,
            false => FileOpType::Move,
        }
    }

    pub fn output_dir(&self) -> &Path {
        self.output_dir.as_deref().unwrap_or(&self.music_dir)
    }
}

fn music_dir_value_parser() -> impl TypedValueParser<Value = PathBuf> {
    clap::builder::StringValueParser::new().try_map(|value| {
        let expanded = shellexpand::tilde(&value);
        let path = PathBuf::from(expanded.into_owned());

        if !path.exists() {
            return Err(format!("`music-dir` doesn't exist: `{}`", path.display()));
        }
        if !path.is_dir() {
            return Err(format!("`music-dir` path isn't a directory: `{}`", path.display()));
        }

        Ok(path)
    })
}

fn output_dir_value_parser() -> impl TypedValueParser<Value = PathBuf> {
    clap::builder::StringValueParser::new().try_map(|value| {
        let expanded = shellexpand::tilde(&value);
        let path = PathBuf::from(expanded.into_owned());

        if path.exists() && !path.is_dir() {
            return Err(format!(
                "`output-dir` path exists but isn't a directory: `{}`",
                path.display()
            ));
        }

        Ok(path)
    })
}

#[derive(Clone, Copy, Default, PartialEq, Eq, clap::ValueEnum)]
pub enum EmbeddedArtworks {
    Keep,
    #[default]
    RemoveRedundant,
    Remove,
}

#[derive(Parser)]
pub struct TranscodeCommand {
    /// The directory which will be searched for music files.
    #[clap(
        long = "music-dir",
        short = 'm',
        value_parser = music_dir_value_parser(),
        default_value = "~/Music",
    )]
    pub music_dir: PathBuf,

    /// The directory which the content will be written to.
    #[clap(
        long = "output-dir",
        short = 'o',
        value_parser = output_dir_value_parser(),
    )]
    pub output_dir: PathBuf,

    /// Verbosity level of the output. 0 means least 2 means most verbose ouput.
    #[clap(
        long = "verbosity",
        short = 'v',
        value_name = "level",
        value_parser = value_parser!(u8).range(0..=2),
        default_value = "1",
    )]
    pub verbosity: u8,

    /// Prints timing information
    #[clap(long = "timings", short = 't')]
    pub timings: bool,
}

#[derive(Parser)]
pub struct CompletionsCommand {
    /// The shell for which the completions are generated.
    #[clap(value_enum)]
    pub shell: Shell,
}
