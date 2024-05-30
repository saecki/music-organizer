use clap::{crate_authors, crate_version, value_parser, Arg, ColorChoice, Command, ValueHint};
use clap_complete::generate;
use clap_complete::shells::{Bash, Elvish, Fish, PowerShell, Zsh};
use music_organizer::FileOpType;
use std::path::PathBuf;
use std::str::FromStr;

const BIN_NAME: &str = "music-organizer";

#[derive(Clone, Copy, PartialEq, Eq)]
enum Shell {
    Bash,
    Elvish,
    Fish,
    Pwrsh,
    Zsh,
}

impl FromStr for Shell {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "bash" => Ok(Shell::Bash),
            "elvish" => Ok(Shell::Elvish),
            "fish" => Ok(Shell::Fish),
            "powershell" => Ok(Shell::Pwrsh),
            "zsh" => Ok(Shell::Zsh),
            _ => Err("Unknown shell"),
        }
    }
}

pub struct Args {
    pub music_dir: PathBuf,
    pub output_dir: PathBuf,
    pub verbosity: u8,
    pub op_type: FileOpType,
    pub assume_yes: bool,
    pub dry_run: bool,
    pub no_check: bool,
    pub keep_embedded_artworks: bool,
    pub no_cleanup: bool,
}

pub fn parse_args() -> Args {
    let mut app = Command::new("music organizer")
        .color(ColorChoice::Auto)
        .version(crate_version!())
        .author(crate_authors!())
        .about("Moves/copies, renames and retags Music files using their metadata.")
        .arg(
            Arg::new("music-dir")
                .short('m')
                .long("music-dir")
                .help("The directory which will be searched for music files")
                .num_args(1)
                .default_value("~/Music")
                .value_hint(ValueHint::DirPath),
        )
        .arg(
            Arg::new("output-dir")
                .short('o')
                .long("output-dir")
                .help("The directory which the content will be written to")
                .num_args(1)
                .value_hint(ValueHint::DirPath),
        )
        .arg(
            Arg::new("copy")
                .short('c')
                .long("copy")
                .help("Copy the files instead of moving")
                .num_args(0)
                .requires("output-dir"),
        )
        .arg(
            Arg::new("nocheck")
                .short('n')
                .long("nocheck")
                .help("Don't check for inconsistencies")
                .num_args(0),
        )
        .arg(
            Arg::new("keep embedded artworks")
                .short('e')
                .long("keep-embedded-artworks")
                .help("Keep embedded artworks")
                .num_args(0),
        )
        .arg(
            Arg::new("nocleanup")
                .long("nocleanup")
                .help("Don't remove empty directories")
                .num_args(0),
        )
        .arg(
            Arg::new("assume-yes")
                .short('y')
                .long("assume-yes")
                .help("Assumes yes as a answer for questions")
                .num_args(0),
        )
        .arg(
            Arg::new("dryrun")
                .short('d')
                .long("dryrun")
                .help("Only check files don't change anything")
                .num_args(0)
                .conflicts_with("assume-yes"),
        )
        .arg(
            Arg::new("verbosity")
                .short('v')
                .long("verbosity")
                .value_name("level")
                .help("Verbosity level of the output. 0 means least 2 means most verbose ouput.")
                .value_parser(value_parser!(u8).range(0..=2))
                .default_value("1"),
        )
        .arg(
            Arg::new("generate-completion")
                .short('g')
                .long("generate-completion")
                .value_name("shell")
                .help("Generates a completion script for the specified shell")
                .conflicts_with("music-dir")
                .value_parser(value_parser!(Shell)),
        );

    let matches = app.clone().get_matches();

    let generate_completion = matches.get_one("generate-completion");
    if let Some(shell) = generate_completion {
        let mut stdout = std::io::stdout();
        match shell {
            Shell::Bash => generate(Bash, &mut app, BIN_NAME, &mut stdout),
            Shell::Elvish => generate(Elvish, &mut app, BIN_NAME, &mut stdout),
            Shell::Fish => generate(Fish, &mut app, BIN_NAME, &mut stdout),
            Shell::Zsh => generate(Zsh, &mut app, BIN_NAME, &mut stdout),
            Shell::Pwrsh => generate(PowerShell, &mut app, BIN_NAME, &mut stdout),
        }
        std::process::exit(0);
    }

    let music_dir = {
        let dir = shellexpand::tilde(matches.get_one::<String>("music-dir").unwrap());
        let path = PathBuf::from(dir.as_ref());
        if !path.exists() {
            println!("Not a valid music dir path: {}", dir);
            std::process::exit(1)
        }
        path
    };

    let output_dir = match matches.get_one::<String>("output-dir") {
        Some(s) => {
            let dir = shellexpand::tilde(s);
            PathBuf::from(dir.as_ref())
        }
        None => music_dir.clone(),
    };

    Args {
        music_dir,
        output_dir,
        verbosity: *matches.get_one::<u8>("verbosity").unwrap(),
        op_type: match matches.get_flag("copy") {
            true => FileOpType::Copy,
            false => FileOpType::Move,
        },
        assume_yes: matches.get_flag("assume-yes"),
        no_check: matches.get_flag("nocheck"),
        keep_embedded_artworks: matches.get_flag("keep embedded artworks"),
        no_cleanup: matches.get_flag("nocleanup"),
        dry_run: matches.get_flag("dryrun"),
    }
}
