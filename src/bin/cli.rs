use clap::{crate_authors, crate_version, App, Arg, ValueHint};
use clap_generate::generate;
use clap_generate::generators::{Bash, Elvish, Fish, PowerShell, Zsh};
use colored::Colorize;

use music_organizer::{
    meta::Metadata,
    meta::{Album, Artist, Song},
    Changes, FileOpType, FileOperation, MusicIndex, OptionAsStr,
};

use std::{
    io::Write,
    path::{Path, PathBuf},
    process::exit,
    str::FromStr,
};

const BIN_NAME: &str = "music_organizer";

const BASH: &str = "bash";
const ELVISH: &str = "elvish";
const FISH: &str = "fish";
const PWRSH: &str = "powershell";
const ZSH: &str = "zsh";

static mut LAST_LEN: usize = 0;

fn main() {
    let mut app = App::new("music organizer")
        .version(crate_version!())
        .author(crate_authors!())
        .about("Moves or copies and renames Music files using their metadata information.")
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

    let verbosity = matches.value_of("verbosity").map(|v| v.parse::<usize>().unwrap()).unwrap_or(0);
    let op_type = match matches.is_present("copy") {
        true => FileOpType::Copy,
        false => FileOpType::Move,
    };
    let yes = matches.is_present("assume-yes");
    let nocheck = matches.is_present("nocheck");
    let dryrun = matches.is_present("dryrun");

    let (op_type_sim_pres, op_type_pres_prog, op_type_sim_past) = match op_type {
        FileOpType::Copy => ("copy", "copying", "copied"),
        FileOpType::Move => ("move", "moving", "moved"),
    };

    println!("============================================================");
    println!("# Indexing");
    println!("============================================================");
    let mut index = MusicIndex::from(music_dir.clone());

    for (i, m) in &mut index.read_iter().enumerate() {
        print_verbose(
            &format!(
                "{} {} {} {}",
                (i + 1).to_string().blue(),
                m.artist.opt_str().green(),
                "-".green(),
                m.title.opt_str().green()
            ),
            verbosity >= 2,
        );
    }
    reset_print_verbose();
    println!();

    let mut changes = Changes::default();

    #[cfg(feature = "checks")]
    {
        if !nocheck {
            println!("============================================================");
            println!("# Checking");
            println!("============================================================");
            changes.check_inconsitent_artists(&index, inconsitent_artists_dialog);
            changes.check_inconsitent_albums(&index, inconsitent_albums_dialog);
            changes.check_inconsitent_total_tracks(&index, inconsitent_total_tracks_dialog);
            changes.check_inconsitent_total_discs(&index, inconsitent_total_discs_dialog);
            println!();
        }
    }

    changes.file_system(&index, &output_dir);

    if changes.dir_creations.is_empty() && changes.file_operations.is_empty() {
        println!("{}", "nothing to do exiting...".green());
        return;
    }
    println!("============================================================");
    println!("# Changes");
    println!("============================================================");

    if verbosity >= 1 {
        if !changes.dir_creations.is_empty() {
            println!("dirs:");
            for (i, d) in changes.dir_creations.iter().enumerate() {
                println!(
                    "{} create {}",
                    (i + 1).to_string().blue(),
                    format!("{}", d.path.display()).green()
                );
            }
            println!();
        }
        if !changes.file_operations.is_empty() {
            println!("files:");
            for (i, f) in changes.file_operations.iter().enumerate() {
                println!(
                    "{} {}",
                    (i + 1).to_string().blue(),
                    format_file_op(&music_dir, &output_dir, f, op_type_sim_pres, verbosity)
                );
            }
            println!();
        }
    }

    println!(
        "{} dirs will be created.\n{} files will be {}.",
        changes.dir_creations.len(),
        changes.file_operations.len(),
        op_type_sim_past,
    );

    if dryrun {
        println!("dryrun, exiting...");
        exit(0);
    } else if !yes {
        let ok = input_confirmation_loop("Continue");

        if !ok {
            println!("exiting...");
            exit(0);
        }
    }

    println!("============================================================");
    println!("# Writing");
    println!("============================================================");
    for (i, (d, r)) in changes.dir_creation_iter().enumerate() {
        match r {
            Ok(_) => {
                print_verbose(
                    &format!("{} created dir {}", (i + 1).to_string().blue(), d.path.display()),
                    verbosity >= 2,
                );
            }
            Err(e) => {
                reset_print_verbose();
                println!(
                    "{} {} creating dir {}:\n{}",
                    (i + 1).to_string().blue(),
                    "error".red(),
                    d.path.display(),
                    e.to_string().red()
                );
            }
        }
    }
    reset_print_verbose();

    for (i, (f, r)) in changes.file_operation_iter(op_type).enumerate() {
        match r {
            Ok(_) => {
                let s = format!(
                    "{} {}",
                    (i + 1).to_string().blue(),
                    format_file_op(&music_dir, &output_dir, f, op_type_sim_past, verbosity)
                );
                print_verbose(&s, verbosity >= 2);
            }
            Err(e) => {
                reset_print_verbose();
                println!(
                    "{} {} {}:\n{}",
                    (i + 1).to_string().blue(),
                    "error".red(),
                    format_file_op(&music_dir, &output_dir, f, op_type_pres_prog, 2),
                    e.to_string().red(),
                );
            }
        }
    }
    reset_print_verbose();

    println!("done");
}

fn inconsitent_artists_dialog(index: &MusicIndex, a: &Artist, b: &Artist) -> Option<String> {
    fn print(index: &MusicIndex, artist: &Artist) {
        println!("{}:", artist.name.as_str().yellow());
        for (i, al) in artist.albums.iter().enumerate() {
            if i == 10 {
                println!("   {}", "...".green());
                break;
            }
            println!("   {}:", al.name);
            for (j, s) in al.songs.iter().map(|&si| &index.songs[si]).enumerate() {
                if i >= 4 || j == 3 {
                    println!("      {}", "...".green());
                    break;
                } else {
                    println!(
                        "      {:02} - {} - {}",
                        s.track_number.unwrap_or(0),
                        s.artist.opt_str(),
                        s.title.opt_str()
                    );
                }
            }
        }
    }
    println!("These two artists are named similarly:");
    print(index, a);
    println!();
    print(index, b);
    println!();

    let index = input_options_loop(
        "",
        &[
            "don't do anything",
            "rename first to second",
            "rename second to first",
            "enter new name",
        ],
    );

    match index {
        0 => return None,
        1 => {
            println!("renaming first to second");
            return Some(a.name.clone());
        }
        2 => {
            println!("renaming second to first");
            return Some(b.name.clone());
        }
        3 => loop {
            let new_name = input_loop("enter new name:", |_| true);
            let msg = format!("new name: '{}'", new_name);

            let i = input_options_loop(&msg, &["ok", "reenter name", "dismiss"]);

            match i {
                0 => return Some(new_name),
                1 => continue,
                _ => return None,
            }
        },
        _ => unreachable!(),
    }
}

fn inconsitent_albums_dialog(
    index: &MusicIndex,
    artist: &Artist,
    a: &Album,
    b: &Album,
) -> Option<String> {
    fn print(index: &MusicIndex, album: &Album) {
        println!("   {}:", album.name.as_str().yellow());
        for s in album.songs.iter().map(|&si| &index.songs[si]) {
            println!(
                "      {:02} - {} - {}",
                s.track_number.unwrap_or(0),
                s.artist.opt_str(),
                s.title.opt_str()
            );
        }
    }
    println!("These two albums are named similarly:");
    println!("{}:", artist.name);
    print(index, a);
    println!();
    print(index, b);
    println!();

    let index = input_options_loop(
        "",
        &[
            "don't do anything",
            "rename first to second",
            "rename second to first",
            "enter new name",
        ],
    );

    match index {
        0 => return None,
        1 => {
            println!("renaming first to second");
            return Some(a.name.clone());
        }
        2 => {
            println!("renaming second to first");
            return Some(b.name.clone());
        }
        3 => loop {
            let new_name = input_loop("enter new name:", |_| true);
            let msg = format!("new name: '{}'", new_name);

            let i = input_options_loop(&msg, &["ok", "reenter name", "dismiss"]);

            match i {
                0 => return Some(new_name),
                1 => continue,
                2 => return None,
                _ => unreachable!(),
            }
        },
        _ => unreachable!(),
    }
}

fn inconsitent_total_tracks_dialog(
    artist: &Artist,
    album: &Album,
    total_tracks: Vec<(Vec<&Song>, Option<u16>)>,
) -> Option<u16> {
    let msg = format!(
        "{} - {} this album has different total tracks values:",
        artist.name.as_str().yellow(),
        album.name.as_str().yellow(),
    );
    let mut options = vec!["don't do anything", "remove the value", "enter a new value"];

    let values: Vec<String> = total_tracks
        .iter()
        .map(|(songs, tt)| {
            let mut tt_str = match tt {
                Some(n) => format!("{:02}:   ", n).yellow().to_string(),
                None => "none: ".yellow().to_string(),
            };
            let mut iter = songs.iter();

            let s = iter.next().unwrap();
            tt_str.push_str(&format!(
                "{}|{:02} - {} - {}",
                &s.disc_number.unwrap_or(0),
                &s.track_number.unwrap_or(0),
                &s.artist.opt_str(),
                &s.title.opt_str()
            ));

            for s in iter {
                tt_str.push_str(&format!(
                    "\n      {}|{:02} - {} - {}",
                    &s.disc_number.unwrap_or(0),
                    &s.track_number.unwrap_or(0),
                    &s.artist.opt_str(),
                    &s.title.opt_str()
                ));
            }

            tt_str
        })
        .collect();

    options.extend(values.iter().map(|s| s.as_str()));

    let i = input_options_loop(&msg, &options);

    match i {
        0 => return None,
        1 => return Some(0),
        2 => loop {
            let new_value = input_loop_parse::<u16>("enter a new value:");
            let msg = format!("new value: '{}'", new_value);

            let i = input_options_loop(&msg, &["ok", "reenter value", "dismiss"]);

            match i {
                0 => return Some(new_value),
                1 => continue,
                _ => return None,
            }
        },
        _ => return total_tracks[i - 3].1,
    }
}

fn inconsitent_total_discs_dialog(
    artist: &Artist,
    album: &Album,
    total_discs: Vec<(Vec<&Song>, Option<u16>)>,
) -> Option<u16> {
    let msg = format!(
        "{} - {} this album has different total discs values:",
        artist.name.as_str().yellow(),
        album.name.as_str().yellow(),
    );
    let mut options = vec!["don't do anything", "remove the value", "enter a new value"];

    let values: Vec<String> = total_discs
        .iter()
        .map(|(songs, tt)| {
            let mut tt_str = match tt {
                Some(n) => format!("{}:    ", n.to_string().yellow()),
                None => "none: ".yellow().to_string(),
            };
            let mut iter = songs.iter();

            let s = iter.next().unwrap();
            tt_str.push_str(&format!(
                "{}|{:02} - {} - {}",
                &s.disc_number.unwrap_or(0),
                &s.track_number.unwrap_or(0),
                &s.artist.opt_str(),
                &s.title.opt_str(),
            ));

            for s in iter {
                tt_str.push_str(&format!(
                    "\n      {}|{:02} - {} - {}",
                    &s.disc_number.unwrap_or(0),
                    &s.track_number.unwrap_or(0),
                    &s.artist.opt_str(),
                    &s.title.opt_str(),
                ));
            }

            tt_str
        })
        .collect();

    options.extend(values.iter().map(|s| s.as_str()));

    let i = input_options_loop(&msg, &options);

    match i {
        0 => None,
        1 => Some(0),
        2 => loop {
            let new_value = input_loop_parse::<u16>("enter a new value:");
            let msg = format!("new value: '{}'", new_value);

            let i = input_options_loop(&msg, &["ok", "reenter value", "dismiss"]);

            match i {
                0 => return Some(new_value),
                1 => continue,
                _ => return None,
            }
        },
        _ => return total_discs[i - 3].1,
    }
}

fn format_file_op(
    music_dir: &Path,
    output_dir: &Path,
    file_op: &FileOperation,
    op_type_str: &str,
    verbosity: usize,
) -> String {
    let old_path = format!("{}", file_op.old.strip_prefix(music_dir).unwrap().display()).yellow();
    let new = file_op
        .new
        .as_ref()
        .map(|n| format!("{}", n.strip_prefix(output_dir).unwrap().display()).green());

    match (&new, &file_op.tag_update) {
        (Some(new_path), Some(tag_update)) => format!(
            "{} {} to {}\n{}",
            op_type_str,
            new_path,
            old_path,
            format_metadata(&tag_update.meta, verbosity)
        ),
        (None, Some(tag_update)) => format_metadata(&tag_update.meta, verbosity),
        (Some(new_path), None) => {
            if op_type_str.len() + old_path.len() + new_path.len() + 5 <= 180 {
                format!("{} {} to {}", op_type_str, old_path, new_path)
            } else {
                format!("{} {}\n    to {}", op_type_str, old_path, new_path)
            }
        }
        (None, None) => format!("Nothing to do: {}", old_path),
    }
}

fn format_metadata(m: &Metadata, verbosity: usize) -> String {
    format!(
        "\
artist: {}
album artist: {}
album: {}
title: {}
track number: {}
total tracks: {}
disc number: {}
total discs: {}
",
        m.artist.as_ref().map(|s| s.as_str()).unwrap_or("unchanged"),
        m.album_artist.as_ref().map(|s| s.as_str()).unwrap_or("unchanged"),
        m.album.as_ref().map(|s| s.as_str()).unwrap_or("unchanged"),
        m.title.as_ref().map(|s| s.as_str()).unwrap_or("unchanged"),
        m.track_number.map(|n| n.to_string()).unwrap_or("unchanged".into()),
        m.total_tracks.map(|n| n.to_string()).unwrap_or("unchanged".into()),
        m.disc_number.map(|n| n.to_string()).unwrap_or("unchanged".into()),
        m.total_discs.map(|n| n.to_string()).unwrap_or("unchanged".into()),
    )
}

#[inline]
fn print_verbose(str: &str, verbose: bool) {
    if verbose {
        println!("{}", str);
    } else {
        let len = str.chars().count();
        let diff = unsafe { LAST_LEN as i32 - len as i32 };

        print!("\r{}", str);
        for _ in 0..diff {
            print!(" ");
        }
        let _ = std::io::stdout().flush().is_ok();

        unsafe {
            LAST_LEN = len;
        }
    }
}

fn reset_print_verbose() {
    println!();
    unsafe {
        LAST_LEN = 0;
    }
}

fn input_loop(str: &str, predicate: fn(&str) -> bool) -> String {
    loop {
        println!("{}", str);
        let mut input = String::new();

        match std::io::stdin().read_line(&mut input) {
            Ok(_) => {
                input.pop();
                if predicate(&input) {
                    return input;
                }
            }
            Err(e) => println!("error:\n {}", e),
        }
    }
}

fn input_loop_parse<T: FromStr + Default>(str: &str) -> T {
    input_loop(str, |v| v.parse::<T>().is_ok()).parse::<T>().unwrap_or_else(|_| unreachable!())
    // Can't use unwrap because FromStr::Err does not neccesarily implement Debug
}

fn input_options_loop(str: &str, options: &[&str]) -> usize {
    loop {
        if !str.is_empty() {
            println!("{}", str);
        }
        let mut input = String::with_capacity(2);

        for (i, s) in options.iter().enumerate() {
            if options.len() < 10 {
                println!("[{}] {}", i, s.replace("\n", "\n    "));
            } else {
                println!("[{:02}] {}", i, s.replace("\n", "\n     "));
            }
        }

        match std::io::stdin().read_line(&mut input) {
            Ok(_) => match usize::from_str(input.trim_matches('\n')) {
                Ok(i) => {
                    if i < options.len() {
                        return i;
                    } else {
                        println!("invalid input")
                    }
                }
                Err(_) => println!("invalid input"),
            },
            Err(e) => println!("error:\n {}", e),
        }
    }
}

fn input_confirmation_loop(str: &str) -> bool {
    loop {
        print!("{} [y/N]?", str);
        let mut input = String::with_capacity(2);

        let _ = std::io::stdout().flush().is_ok();

        if let Err(e) = std::io::stdin().read_line(&mut input) {
            println!("error:\n {}", e);
        } else {
            input.make_ascii_lowercase();

            if input == "\n" || input == "n\n" {
                return false;
            } else if input == "y\n" {
                return true;
            } else {
                println!("invalid input");
            }
        }
    }
}
