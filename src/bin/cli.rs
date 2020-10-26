use clap::{App, Arg, Shell};
use colorful::Colorful;
use music_organizer::{Album, Artist, Changes, FileOpType, MusicIndex, OptionAsStr, Song};
use std::io::Write;
use std::path::PathBuf;
use std::process::exit;
use std::str::FromStr;

static mut LAST_LEN: usize = 0;

fn main() {
    let mut app = App::new("music organizer")
        .version("0.1.0")
        .author("Saecki")
        .about("Moves or copies and renames Music files using their metadata information.")
        .arg(
            Arg::with_name("music-dir")
                .short("m")
                .long("music-dir")
                .help("The directory which will be searched for music files")
                .takes_value(true)
                .required_unless("generate-completion")
                .conflicts_with("generate-completion"),
        )
        .arg(
            Arg::with_name("output-dir")
                .short("o")
                .long("output-dir")
                .help("The directory which the content will be written to")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("copy")
                .short("c")
                .long("copy")
                .help("Copy the files instead of moving")
                .requires("output-dir"),
        )
        .arg(
            Arg::with_name("assume-yes")
                .short("y")
                .long("assume-yes")
                .help("Assumes yes as a answer for all questions")
                .takes_value(false),
        )
        .arg(
            Arg::with_name("dryrun")
                .short("d")
                .long("dryrun")
                .help("Only check files don't change anything")
                .takes_value(false)
                .conflicts_with("assume-yes"),
        )
        .arg(
            Arg::with_name("verbosity")
                .short("v")
                .long("verbosity")
                .value_name("level")
                .help("Verbosity level of the output. 0 means least 2 means most verbose ouput.")
                .takes_value(true)
                .possible_values(&["0", "1", "2"])
                .default_value("1"),
        )
        .arg(
            Arg::with_name("generate-completion")
                .short("g")
                .long("generate-completion")
                .value_name("shell")
                .help("Generates a completion script for the specified shell")
                .conflicts_with("music-dir")
                .requires("output-dir")
                .takes_value(true)
                .possible_values(&["bash", "zsh", "fish", "elvish", "powershell"]),
        );

    let matches = app.clone().get_matches();
    let generate_completion = matches.value_of("generate-completion").unwrap_or("");

    if generate_completion != "" {
        let output_dir = PathBuf::from(matches.value_of("output-dir").unwrap());
        if !output_dir.exists() {
            match std::fs::create_dir_all(&output_dir) {
                Ok(_) => println!("created dir: {}", output_dir.display()),
                Err(e) => println!("error creating dir: {}\n{}", output_dir.display(), e),
            }
        }

        println!("generating completions...");
        let shell = Shell::from_str(generate_completion).unwrap();
        app.gen_completions("music_organizer", shell, output_dir);
        println!("done");
        exit(0);
    }
    let music_dir = PathBuf::from(matches.value_of("music-dir").unwrap());
    let abs_music_dir = match PathBuf::from(&music_dir).canonicalize() {
        Ok(t) => t,
        Err(e) => {
            println!(
                "Not a valid music dir path: {}\n{:?}",
                music_dir.display(),
                e
            );
            exit(1)
        }
    };

    let output_dir = match matches.value_of("output-dir") {
        Some(s) => PathBuf::from(s),
        None => abs_music_dir.clone(),
    };

    let verbosity = matches
        .value_of("verbosity")
        .map(|v| v.parse::<usize>().unwrap())
        .unwrap_or(0);
    let op_type = match matches.is_present("copy") {
        true => FileOpType::Copy,
        false => FileOpType::Move,
    };
    let yes = matches.is_present("assume-yes");
    let dryrun = matches.is_present("dryrun");

    let (op_type_str_present, op_type_str_past) = match op_type {
        FileOpType::Copy => ("copy", "copied"),
        FileOpType::Move => ("move", "moved"),
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

    println!("============================================================");
    println!("# Checking");
    println!("============================================================");
    music_organizer::check_inconsitent_artists(&mut index, inconsitent_artists_dialog);
    music_organizer::check_inconsitent_albums(&mut index, inconsitent_albums_dialog);
    music_organizer::check_inconsitent_total_tracks(&mut index, inconsitent_total_tracks_dialog);
    music_organizer::check_inconsitent_total_discs(&mut index, inconsitent_total_discs_dialog);

    println!();

    let changes = Changes::from(&index, &output_dir);
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
                    "{} {} {} to {}",
                    (i + 1).to_string().blue(),
                    op_type_str_present,
                    format!("{}", f.old.strip_prefix(&music_dir).unwrap().display()).yellow(),
                    format!("{}", f.new.strip_prefix(&output_dir).unwrap().display()).green(),
                );
            }
            println!();
        }
    }

    println!(
        "{} dirs will be created.\n{} files will be {}.",
        changes.dir_creations.len(),
        changes.file_operations.len(),
        op_type_str_past,
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
                    &format!(
                        "{} created dir {}",
                        (i + 1).to_string().blue(),
                        d.path.display()
                    ),
                    verbosity >= 2,
                );
            }
            Err(e) => {
                reset_print_verbose();
                println!(
                    "{} error creating dir {}:\n{}",
                    (i + 1).to_string().blue(),
                    d.path.display(),
                    e
                );
            }
        }
    }
    reset_print_verbose();

    for (i, (f, r)) in changes.file_operation_iter(op_type).enumerate() {
        match r {
            Ok(_) => {
                print_verbose(
                    &format!(
                        "{} {} {} to {}",
                        (i + 1).to_string().blue(),
                        op_type_str_past,
                        format!("{}", f.old.strip_prefix(&music_dir).unwrap().display()).yellow(),
                        format!("{}", f.new.strip_prefix(&output_dir).unwrap().display()).green(),
                    ),
                    verbosity >= 2,
                );
            }
            Err(e) => {
                reset_print_verbose();
                println!(
                    "{} {} {} {} to {}:\n{}",
                    (i + 1).to_string().blue(),
                    "error.red".red(),
                    op_type_str_past,
                    format!("{}", f.old.strip_prefix(&music_dir).unwrap().display()).yellow(),
                    format!("{}", f.new.strip_prefix(&output_dir).unwrap().display()).green(),
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
                        s.track.unwrap_or(0),
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
                s.track.unwrap_or(0),
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
                Some(n) => format!("{:02}:   ", n.to_string().yellow()),
                None => "none: ".yellow().to_string(),
            };
            let mut iter = songs.iter();

            let s = iter.next().unwrap();
            tt_str.push_str(&format!(
                "{:02} - {} - {}",
                &s.track.unwrap_or(0),
                &s.artist.opt_str(),
                &s.title.opt_str()
            ));

            for s in iter {
                tt_str.push_str(&format!(
                    "\n      {:02} - {} - {}",
                    &s.track.unwrap_or(0),
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
                Some(n) => format!("{:02}:   ", n.to_string().yellow()),
                None => "none: ".yellow().to_string(),
            };
            let mut iter = songs.iter();

            let s = iter.next().unwrap();
            tt_str.push_str(&format!(
                "{:02} - {} - {}",
                &s.disc.unwrap_or(0),
                &s.artist.opt_str(),
                &s.title.opt_str()
            ));

            for s in iter {
                tt_str.push_str(&format!(
                    "\n      {:02} - {} - {}",
                    &s.disc.unwrap_or(0),
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
        _ => return total_discs[i - 3].1,
    }
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
    input_loop(str, |v| v.parse::<T>().is_ok())
        .parse::<T>()
        .unwrap_or_else(|_| unreachable!()) // Can't use unwrap because FromStr::Err does not neccesarily implement Debug
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