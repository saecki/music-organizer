use clap::{crate_authors, crate_version, App, Arg, ValueHint};
use clap_generate::generate;
use clap_generate::generators::{Bash, Elvish, Fish, PowerShell, Zsh};
use music_organizer::FileOpType;
use std::path::PathBuf;
use std::process::exit;

const BIN_NAME: &str = "music-organizer";

const BASH: &str = "bash";
const ELVISH: &str = "elvish";
const FISH: &str = "fish";
const PWRSH: &str = "powershell";
const ZSH: &str = "zsh";

pub struct Args {
    pub music_dir: PathBuf,
    pub output_dir: PathBuf,
    pub verbosity: usize,
    pub op_type: FileOpType,
    pub assume_yes: bool,
    pub dry_run: bool,
    pub no_check: bool,
    pub no_cleanup: bool,
}

pub fn parse_args() -> Args {
    let mut app = App::new("music organizer")
        .version(crate_version!())
        .author(crate_authors!())
        .about("Moves/copies, renames and retags Music files using their metadata.")
        .arg(
            Arg::new("music-dir")
                .short('m')
                .long("music-dir")
                .about("The directory which will be searched for music files")
                .takes_value(true)
                .required_unless_present("generate-completion")
                .value_hint(ValueHint::DirPath),
        )
        .arg(
            Arg::new("output-dir")
                .short('o')
                .long("output-dir")
                .about("The directory which the content will be written to")
                .takes_value(true)
                .value_hint(ValueHint::DirPath),
        )
        .arg(
            Arg::new("copy")
                .short('c')
                .long("copy")
                .about("Copy the files instead of moving")
                .requires("output-dir"),
        )
        .arg(
            Arg::new("nocheck")
                .short('n')
                .long("nocheck")
                .about("Don't check for inconsistencies")
                .takes_value(false),
        )
        .arg(
            Arg::new("nocleanup")
                .long("nocleanup")
                .about("Don't remove empty directories")
                .takes_value(false),
        )
        .arg(
            Arg::new("assume-yes")
                .short('y')
                .long("assume-yes")
                .about("Assumes yes as a answer for questions")
                .takes_value(false),
        )
        .arg(
            Arg::new("dryrun")
                .short('d')
                .long("dryrun")
                .about("Only check files don't change anything")
                .takes_value(false)
                .conflicts_with("assume-yes"),
        )
        .arg(
            Arg::new("verbosity")
                .short('v')
                .long("verbosity")
                .value_name("level")
                .about("Verbosity level of the output. 0 means least 2 means most verbose ouput.")
                .takes_value(true)
                .possible_values(&["0", "1", "2"])
                .default_value("1"),
        )
        .arg(
            Arg::new("generate-completion")
                .short('g')
                .long("generate-completion")
                .value_name("shell")
                .about("Generates a completion script for the specified shell")
                .conflicts_with("music-dir")
                .takes_value(true)
                .possible_values(&[BASH, ZSH, FISH, ELVISH, PWRSH]),
        );

    let matches = app.clone().get_matches();

    let generate_completion = matches.value_of("generate-completion");
    if let Some(shell) = generate_completion {
        let mut stdout = std::io::stdout();
        match shell {
            BASH => generate::<Bash, _>(&mut app, BIN_NAME, &mut stdout),
            ELVISH => generate::<Elvish, _>(&mut app, BIN_NAME, &mut stdout),
            FISH => generate::<Fish, _>(&mut app, BIN_NAME, &mut stdout),
            ZSH => generate::<Zsh, _>(&mut app, BIN_NAME, &mut stdout),
            PWRSH => generate::<PowerShell, _>(&mut app, BIN_NAME, &mut stdout),
            _ => unreachable!(),
        }
        exit(0);
    }

    let music_dir = {
        let dir = PathBuf::from(matches.value_of("music-dir").unwrap());
        match PathBuf::from(&dir).canonicalize() {
            Ok(t) => t,
            Err(e) => {
                println!("Not a valid music dir path: {}\n{:?}", dir.display(), e);
                exit(1)
            }
        }
    };

    let output_dir = match matches.value_of("output-dir") {
        Some(s) => {
            let dir = PathBuf::from(s);
            match dir.canonicalize() {
                Ok(p) => p,
                Err(_) => std::env::current_dir()
                    .map(|wd| wd.join(dir.clone()))
                    .expect("could not retrieve working directory"),
            }
        }
        None => music_dir.clone(),
    };

    Args {
        music_dir,
        output_dir,
        verbosity: matches.value_of("verbosity").map(|v| v.parse::<usize>().unwrap()).unwrap_or(1),
        op_type: match matches.is_present("copy") {
            true => FileOpType::Copy,
            false => FileOpType::Move,
        },
        assume_yes: matches.is_present("assume-yes"),
        no_check: matches.is_present("nocheck"),
        no_cleanup: matches.is_present("nocleanup"),
        dry_run: matches.is_present("dryrun"),
    }
}
