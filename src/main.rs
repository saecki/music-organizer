use clap::{App, Arg, Shell};
use colorful::Colorful;
use music_organizer::{Changes, FileOpType, MusicIndex, OptionAsStr};
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
                m.artist.as_str().green(),
                "-".green(),
                m.title.as_str().green()
            ),
            verbosity >= 2,
        );
    }
    reset_print_verbose();
    println!();

    println!("============================================================");
    println!("# Checking");
    println!("============================================================");
    music_organizer::check_inconsitent_artists(&mut index, |a, b| {
        println!(
            "These two artists are named similarly:\n{}\n{}",
            a.name, b.name,
        );
        let index = input_options_loop(&[
            "don't do anything",
            "merge using first",
            "merge using second",
            "enter new name",
        ]);

        match index {
            0 => return None,
            1 => {
                println!("merging using first");
                return Some(a.name.clone());
            }
            2 => {
                println!("merging using second");
                return Some(b.name.clone());
            }
            3 => loop {
                let new_name = input_loop("enter new name:", |_| true);
                println!("new name: '{}'", new_name);

                let i = input_options_loop(&["ok", "reenter name", "dismiss"]);

                match i {
                    0 => return Some(new_name),
                    1 => continue,
                    _ => return None,
                }
            },
            _ => unreachable!(),
        }
    });
    println!();

    music_organizer::check_inconsitent_total_tracks(&mut index, |ar, al, total_tracks| {
        println!(
            "{} - {} this album has different total track values:",
            ar.name.as_str(),
            al.name.as_str()
        );

        let mut options = vec!["don't do anything", "remove the value", "enter a new value"];

        let values: Vec<String> = total_tracks
            .iter()
            .map(|t| match t {
                Some(n) => format!("{:02}", n),
                None => "none".to_string(),
            })
            .collect();

        options.extend(values.iter().map(|s| s.as_str()));

        let i = input_options_loop(&options);

        match i {
            0 => return None,
            1 => return Some(0),
            2 => loop {
                let new_value = input_loop_parse::<u16>("enter a new value:");
                println!("new value: '{}'", new_value);

                let i = input_options_loop(&["ok", "reenter value", "dismiss"]);

                match i {
                    0 => return Some(new_value),
                    1 => continue,
                    _ => return None,
                }
            },
            _ => return total_tracks[i - 3],
        }
    });
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
                    i + 1,
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
                    i + 1,
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
                    &format!("{} created dir {}", i + 1, d.path.display()),
                    verbosity >= 2,
                );
            }
            Err(e) => {
                reset_print_verbose();
                println!("{} error creating dir {}:\n{}", i + 1, d.path.display(), e);
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
                        i + 1,
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
                    i + 1,
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
        .unwrap_or(T::default()) // Unreachable default
}

fn input_options_loop(options: &[&str]) -> usize {
    let mut input = String::with_capacity(2);

    loop {
        for (i, s) in options.iter().enumerate() {
            if options.len() < 10 {
                println!("[{}] {}", i, s);
            } else {
                println!("[{:02}] {}", i, s);
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
    let mut input = String::with_capacity(2);

    loop {
        print!("{} [y/N]?", str);
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
