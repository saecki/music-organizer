use colored::Colorize;
use music_organizer::{Changes, Checks, Cleanup, FileOpType, MusicIndex, ReleaseArtists, Value};
use std::fmt::Write as _;
use std::io::Write as _;

use crate::args::Args;
use crate::display::strip_dir;

mod args;
mod display;

const VERBOSE: u8 = 2;
const MAX_TITLE_WITH: usize = 9;
const TITLE_INDEXING: &str = "INDEXING";
const TITLE_CHECKING: &str = "CHECKING";
const TITLE_CHANGES: &str = "CHANGES";
const TITLE_WRITING: &str = "WRITING";
const TITLE_CLEANUP: &str = "CLEANUP";
const TITLE_DELETIONS: &str = "DELETIONS";
const TITLE_CLEANING: &str = "CLEANING";

const MAX_SUBTITLE_WITH: usize = 6;
const SUBTITLE_DIRS: &str = "dirs";
const SUBTITLE_SONGS: &str = "songs";
const SUBTITLE_OTHERS: &str = "others";

const RENAME_TENSES: Tenses =
    Tenses { sim_pres: "rename", pres_prog: "renaming", sim_past: "renamed" };
const MOVE_TENSES: Tenses = Tenses { sim_pres: "move", pres_prog: "moving", sim_past: "moved" };
const COPY_TENSES: Tenses = Tenses { sim_pres: "copy", pres_prog: "copying", sim_past: "copied" };

struct Dict {
    op_type: Tenses,
    rename: Tenses,
}

struct Tenses {
    sim_pres: &'static str,
    pres_prog: &'static str,
    sim_past: &'static str,
}

fn print_title_verbose(verbose: bool, title: &str) {
    if verbose {
        print_title(title)
    }
}

fn print_title(title: &str) {
    let padding = MAX_TITLE_WITH - title.len() + 1;
    println!("{} ", format!(" {title}{:padding$}", "").purple().on_black());
}

fn print_subtitle(title: &str) {
    let padding = MAX_SUBTITLE_WITH - title.len() + 1;
    println!("{} ", format!(" {title}{:padding$}", "").cyan().on_black());
}

macro_rules! print_verbose {
    ($verbose:expr, $title:expr, $pat:expr, $($args:expr),*) => {{
        if $verbose {
            println!($pat $(,$args)*);
        } else {
            print!("\x1b[2K\r");
            let padding = MAX_TITLE_WITH - $title.len() + 1;
            print!("{} ", format!(" {}{:padding$}", $title, "").purple().on_black());
            print!($pat $(,$args)*);
            std::io::stdout().flush().ok();
        }
    }}
}

fn main() {
    let args = args::parse_args();
    let dict = Dict {
        op_type: match args.op_type {
            FileOpType::Move => MOVE_TENSES,
            FileOpType::Copy => COPY_TENSES,
        },
        rename: RENAME_TENSES,
    };

    // indexing
    let mut index = MusicIndex::from(args.music_dir.clone());
    display_indexing(&mut index, &args);

    // checking
    let mut checks = Checks::from(&index);
    if !args.no_check {
        display_checking(&mut checks, &args);
    }

    // changes
    let changes = Changes::generate(checks, &args.output_dir);
    display_changes(&changes, &args, &dict);

    if !changes.is_empty() {
        // writing
        if !args.assume_yes && !args.dry_run {
            let ok = confirm_input("continue");
            if !ok {
                successfull_early_exit();
            }
        }
        display_writing(&changes, &args, &dict)
    }

    if !args.no_cleanup {
        // cleanup
        let mut cleanup = Cleanup::from(args.music_dir.clone());
        display_cleanup(&mut cleanup, &args);

        // deletions
        display_deletions(&cleanup, &args);

        if !cleanup.is_empty() {
            // cleaning
            if !args.assume_yes && !args.dry_run {
                let ok = confirm_input("continue");
                if !ok {
                    successfull_early_exit();
                }
            }
            display_cleaning(&cleanup, &args);
        }
    }
}

fn display_indexing(index: &mut MusicIndex, args: &Args) {
    let verbose = args.verbosity >= 2;
    print_title_verbose(verbose, TITLE_INDEXING);

    let mut i = 1;
    index.read(&mut |p| {
        print_verbose!(
            verbose,
            TITLE_INDEXING,
            "{} {}",
            i.to_string().blue(),
            strip_dir(p, &args.music_dir).yellow()
        );
        i += 1;
    });
    if !verbose {
        print_verbose!(
            verbose,
            TITLE_INDEXING,
            "{} {}",
            (i - 1).to_string().blue(),
            "files indexed".green()
        );
    }
    println!();
}

fn display_checking(checks: &mut Checks, args: &Args) {
    let verbose = args.verbosity >= 2;
    print_title_verbose(verbose, TITLE_CHECKING);

    if !args.keep_embedded_artworks {
        print_verbose!(verbose, TITLE_CHECKING, "{}", "embedded artworks".yellow());
        checks.remove_embedded_artworks();
    }

    print_verbose!(verbose, TITLE_CHECKING, "{}", "file permissions".yellow());
    checks.check_file_permissions();

    print_verbose!(verbose, TITLE_CHECKING, "{}", "inconsistent artists".yellow());
    checks.check_inconsitent_release_artists(inconsitent_artists_dialog);
    //changes.check_inconsitent_albums(inconsitent_albums_dialog);
    //changes.check_inconsitent_total_tracks(inconsitent_total_tracks_dialog);
    //changes.check_inconsitent_total_discs(inconsitent_total_discs_dialog);

    if !verbose {
        print_verbose!(verbose, TITLE_CHECKING, "{}", "done".green());
    }

    println!();
}

fn display_changes(changes: &Changes, args: &Args, dict: &Dict) {
    if changes.is_empty() {
        let verbose = args.verbosity >= 2;
        print_title_verbose(verbose, TITLE_CHANGES);
        print_verbose!(verbose, TITLE_CHANGES, "{}\n", "nothing to do".green());
        return;
    }

    let verbose = args.verbosity >= 1;
    print_title_verbose(verbose, TITLE_CHANGES);

    if verbose {
        if !changes.dir_creations.is_empty() {
            print_subtitle(SUBTITLE_DIRS);
            for (i, d) in changes.dir_creations.iter().enumerate() {
                println!(
                    "{} create {}",
                    (i + 1).to_string().blue(),
                    format!("{}", d.path.display()).yellow()
                );
            }
            println!();
        }
        if !changes.song_operations.is_empty() {
            print_subtitle(SUBTITLE_SONGS);
            for (i, o) in changes.song_operations.iter().enumerate() {
                println!(
                    "{} {}",
                    (i + 1).to_string().blue(),
                    display::SongOp(
                        &args.music_dir,
                        &args.output_dir,
                        o,
                        dict.op_type.sim_pres,
                        dict.rename.sim_pres,
                        args.verbosity,
                    )
                );
            }
            println!();
        }
        if !changes.file_operations.is_empty() {
            print_subtitle(SUBTITLE_OTHERS);
            for (i, f) in changes.file_operations.iter().enumerate() {
                println!(
                    "{} {}",
                    (i + 1).to_string().blue(),
                    display::FileOp(
                        &args.music_dir,
                        &args.output_dir,
                        f.old_path,
                        &f.new_path,
                        dict.op_type.sim_pres,
                        dict.rename.sim_pres,
                    )
                );
            }
            println!();
        }
    }

    let num_dir_creations = changes.dir_creations.len();
    let num_file_ops = changes.song_operations.len() + changes.file_operations.len();
    print_verbose!(
        verbose,
        TITLE_CHANGES,
        "{} {} will be created{}{} {} will be {}",
        num_dir_creations.to_string().blue(),
        if num_dir_creations == 1 { "dir" } else { "dirs" },
        if verbose { '\n' } else { ' ' },
        num_file_ops.to_string().blue(),
        if num_file_ops == 1 { "file" } else { "files" },
        dict.op_type.sim_past
    );

    println!();
}

fn display_writing(changes: &Changes, args: &Args, dict: &Dict) {
    if args.dry_run {
        println!("skip writing dryrun...");
        return;
    }

    let verbose = args.verbosity >= 2;
    print_title_verbose(verbose, TITLE_WRITING);

    let mut dir_creation_idx = 1;
    changes.execute_dir_creations(&mut |d, r| {
        match r {
            Ok(_) => {
                print_verbose!(
                    verbose,
                    TITLE_WRITING,
                    "{} created dir {}",
                    dir_creation_idx.to_string().blue(),
                    d.path.display()
                );
            }
            Err(e) => {
                print_verbose!(
                    false,
                    TITLE_WRITING,
                    "{} {} creating dir {}: {}\n",
                    dir_creation_idx.to_string().blue(),
                    "error".red(),
                    d.path.display(),
                    e.to_string().red()
                );
            }
        }

        dir_creation_idx += 1;
    });

    let mut file_operation_idx = 1;
    changes.execute_song_operations(args.op_type, &mut |o, r| {
        match r {
            Ok(_) => {
                let display_obj = display::SongOp(
                    &args.music_dir,
                    &args.output_dir,
                    o,
                    dict.op_type.sim_past,
                    dict.rename.sim_past,
                    args.verbosity,
                );
                print_verbose!(
                    verbose,
                    TITLE_WRITING,
                    "{} {}",
                    file_operation_idx.to_string().blue(),
                    display_obj
                );
            }
            Err(e) => {
                println!(
                    "{} {} {}:\n{}",
                    file_operation_idx.to_string().blue(),
                    "error".red(),
                    display::SongOp(
                        &args.music_dir,
                        &args.output_dir,
                        o,
                        dict.op_type.pres_prog,
                        dict.rename.pres_prog,
                        VERBOSE
                    ),
                    e.to_string().red(),
                );
            }
        }

        file_operation_idx += 1;
    });

    changes.execute_file_operations(args.op_type, &mut |f, r| {
        match r {
            Ok(_) => {
                let display_obj = display::FileOp(
                    &args.music_dir,
                    &args.output_dir,
                    f.old_path,
                    &f.new_path,
                    dict.op_type.sim_past,
                    dict.rename.sim_past,
                );
                print_verbose!(
                    verbose,
                    TITLE_WRITING,
                    "{} {}",
                    file_operation_idx.to_string().blue(),
                    display_obj
                );
            }
            Err(e) => {
                print!(
                    "{} {} {}:\n{}",
                    file_operation_idx.to_string().blue(),
                    "error".red(),
                    display::FileOp(
                        &args.music_dir,
                        &args.output_dir,
                        f.old_path,
                        &f.new_path,
                        dict.op_type.pres_prog,
                        dict.rename.pres_prog,
                    ),
                    e.to_string().red(),
                );
            }
        }

        file_operation_idx += 1;
    });

    if !verbose {
        let num_dir_creations = dir_creation_idx - 1;
        let num_file_ops = file_operation_idx - 1;
        print_verbose!(
            verbose,
            TITLE_WRITING,
            "{} {} {} {} {}",
            num_dir_creations.to_string().blue(),
            if num_dir_creations == 1 { "dir created" } else { "dirs created" }.green(),
            num_file_ops.to_string().blue(),
            if num_file_ops == 1 { "file" } else { "files" }.green(),
            dict.op_type.sim_past.green()
        );
    }

    println!();
}

fn display_cleanup(cleanup: &mut Cleanup, args: &Args) {
    let verbose = args.verbosity >= 2;
    print_title_verbose(verbose, TITLE_CLEANUP);

    let mut i = 1;
    cleanup.check(&mut |p| {
        print_verbose!(
            verbose,
            TITLE_CLEANUP,
            "{} {}",
            i.to_string().blue(),
            strip_dir(p, &args.music_dir).yellow()
        );

        i += 1;
    });

    if !verbose {
        print_verbose!(
            verbose,
            TITLE_CLEANUP,
            "{} {}",
            (i - 1).to_string().blue(),
            "dirs checked".green()
        );
    }

    println!();
}

fn display_deletions(cleanup: &Cleanup, args: &Args) {
    if cleanup.is_empty() {
        let verbose = args.verbosity >= 2;
        print_title_verbose(verbose, TITLE_DELETIONS);
        print_verbose!(verbose, TITLE_DELETIONS, "{}\n", "nothing to cleanup".green());
    } else {
        let verbose = args.verbosity >= 1;
        print_title_verbose(verbose, TITLE_DELETIONS);

        if verbose {
            print_subtitle(SUBTITLE_DIRS);

            for (i, d) in cleanup.dir_deletions.iter().enumerate() {
                println!(
                    "{} delete {}",
                    (i + 1).to_string().blue(),
                    strip_dir(&d.path, &args.music_dir).red(),
                );
            }
            println!();
        }

        let num_dir_deletions = cleanup.dir_deletions.len();
        print_verbose!(
            verbose,
            TITLE_DELETIONS,
            "{} {} will be deleted",
            num_dir_deletions.to_string().blue(),
            if num_dir_deletions == 1 { "dir" } else { "dirs" }
        );

        println!();
    }
}

fn display_cleaning(cleanup: &Cleanup, args: &Args) {
    if args.dry_run {
        println!("skip cleaning up dryrun...");
    } else {
        let verbose = args.verbosity >= 2;
        print_title_verbose(verbose, TITLE_CLEANING);

        let mut i = 1;
        cleanup.excecute(&mut |p| {
            print_verbose!(
                verbose,
                TITLE_CLEANING,
                "{} deleted {}",
                i.to_string().blue(),
                strip_dir(p, &args.music_dir).red()
            );
            i += 1;
        });

        if !verbose {
            print_verbose!(
                verbose,
                TITLE_CLEANING,
                "{} {}",
                (i - 1).to_string().blue(),
                if i == 1 { "dir deleted" } else { "dirs deleted" }.green()
            );
        }
        println!();
    }
}

fn inconsitent_artists_dialog(a: &ReleaseArtists, b: &ReleaseArtists) -> Value<Vec<String>> {
    fn print(artist: &ReleaseArtists) {
        for n in artist.names {
            println!(" {}", n.yellow().on_black());
        }
        println!();
        for (i, al) in artist.releases.iter().enumerate() {
            if i == 10 {
                println!("   {}", "...".green());
                break;
            }
            println!("   {}:", al.name);
            for (j, s) in al.songs.iter().enumerate() {
                if i >= 4 || j == 3 {
                    println!("      {}", "...".green());
                    break;
                } else {
                    println!(
                        "      {:02} - {} - {}",
                        s.track_number.unwrap_or(0),
                        artist.names.join(", "),
                        s.title
                    );
                }
            }
        }
    }
    println!("\nThese two artists are named similarly:");
    print(a);
    println!();
    print(b);
    println!();

    let index = options_input(
        "",
        &[
            "don't do anything",
            "rename first to second",
            "rename second to first",
            "enter new name[s]",
        ],
    );

    match index {
        0 => Value::Unchanged,
        1 => {
            println!("renaming first to second");
            Value::Update(b.names.to_vec())
        }
        2 => {
            println!("renaming second to first");
            Value::Update(a.names.to_vec())
        }
        3 => {
            let mut new_names = Vec::new();
            loop {
                new_names.push(string_input("enter new name:"));
                let mut msg = String::from("new name[s]:");
                for n in new_names.iter() {
                    _ = write!(msg, " {}", n.green().on_black());
                }

                let i = options_input(&msg, &["ok", "reenter name", "add another", "dismiss"]);
                match i {
                    0 => return Value::Update(new_names),
                    1 => {
                        new_names.pop();
                        continue;
                    }
                    2 => continue,
                    _ => return Value::Unchanged,
                }
            }
        }
        _ => unreachable!(),
    }
}

//fn inconsitent_albums_dialog(
//    index: &MusicIndex,
//    artist: &ReleaseArtists,
//    a: &Release,
//    b: &Release,
//) -> Option<String> {
//    fn print(index: &MusicIndex, album: &Release) {
//        println!("   {}:", album.name.as_str().yellow());
//        for s in album.songs.iter().map(|&si| &index.songs[si]) {
//            println!(
//                "      {:02} - {} - {}",
//                s.track_number.unwrap_or(0),
//                s.artist.opt_str(),
//                s.title.opt_str()
//            );
//        }
//    }
//    println!("These two albums are named similarly:");
//    println!("{}:", artist.name);
//    print(index, a);
//    println!();
//    print(index, b);
//    println!();
//
//    let index = input_options_loop(
//        "",
//        &[
//            "don't do anything",
//            "rename first to second",
//            "rename second to first",
//            "enter new name",
//        ],
//    );
//
//    match index {
//        0 => return None,
//        1 => {
//            println!("renaming first to second");
//            return Some(a.name.clone());
//        }
//        2 => {
//            println!("renaming second to first");
//            return Some(b.name.clone());
//        }
//        3 => loop {
//            let new_name = input_loop("enter new name:", |_| true);
//            let msg = format!("new name: '{}'", new_name);
//
//            let i = input_options_loop(&msg, &["ok", "reenter name", "dismiss"]);
//
//            match i {
//                0 => return Some(new_name),
//                1 => continue,
//                2 => return None,
//                _ => unreachable!(),
//            }
//        },
//        _ => unreachable!(),
//    }
//}
//
//fn inconsitent_total_tracks_dialog(
//    artist: &ReleaseArtists,
//    album: &Release,
//    total_tracks: Vec<(Vec<&Song>, Option<u16>)>,
//) -> Option<u16> {
//    let msg = format!(
//        "{} - {} this album has different total tracks values:",
//        artist.name.as_str().yellow(),
//        album.name.as_str().yellow(),
//    );
//    let mut options = vec!["don't do anything", "remove the value", "enter a new value"];
//
//    let values: Vec<String> = total_tracks
//        .iter()
//        .map(|(songs, tt)| {
//            let mut tt_str = match tt {
//                Some(n) => format!("{:02}:   ", n).yellow().to_string(),
//                None => "none: ".yellow().to_string(),
//            };
//            let mut iter = songs.iter();
//
//            let s = iter.next().unwrap();
//            tt_str.push_str(&format!(
//                "{}|{:02} - {} - {}",
//                &s.disc_number.unwrap_or(0),
//                &s.track_number.unwrap_or(0),
//                &s.artist.opt_str(),
//                &s.title.opt_str()
//            ));
//
//            for s in iter {
//                tt_str.push_str(&format!(
//                    "\n      {}|{:02} - {} - {}",
//                    &s.disc_number.unwrap_or(0),
//                    &s.track_number.unwrap_or(0),
//                    &s.artist.opt_str(),
//                    &s.title.opt_str()
//                ));
//            }
//
//            tt_str
//        })
//        .collect();
//
//    options.extend(values.iter().map(|s| s.as_str()));
//
//    let i = input_options_loop(&msg, &options);
//
//    match i {
//        0 => return None,
//        1 => return Some(0),
//        2 => loop {
//            let new_value = input_loop_parse::<u16>("enter a new value:");
//            let msg = format!("new value: '{}'", new_value);
//
//            let i = input_options_loop(&msg, &["ok", "reenter value", "dismiss"]);
//
//            match i {
//                0 => return Some(new_value),
//                1 => continue,
//                _ => return None,
//            }
//        },
//        _ => return total_tracks[i - 3].1,
//    }
//}
//
//fn inconsitent_total_discs_dialog(
//    artist: &ReleaseArtists,
//    album: &Release,
//    total_discs: Vec<(Vec<&Song>, Option<u16>)>,
//) -> Option<u16> {
//    let msg = format!(
//        "{} - {} this album has different total discs values:",
//        artist.name.as_str().yellow(),
//        album.name.as_str().yellow(),
//    );
//    let mut options = vec!["don't do anything", "remove the value", "enter a new value"];
//
//    let values: Vec<String> = total_discs
//        .iter()
//        .map(|(songs, tt)| {
//            let mut tt_str = match tt {
//                Some(n) => format!("{}:    ", n.to_string().yellow()),
//                None => "none: ".yellow().to_string(),
//            };
//            let mut iter = songs.iter();
//
//            let s = iter.next().unwrap();
//            tt_str.push_str(&format!(
//                "{}|{:02} - {} - {}",
//                &s.disc_number.unwrap_or(0),
//                &s.track_number.unwrap_or(0),
//                &s.artist.opt_str(),
//                &s.title.opt_str(),
//            ));
//
//            for s in iter {
//                tt_str.push_str(&format!(
//                    "\n      {}|{:02} - {} - {}",
//                    &s.disc_number.unwrap_or(0),
//                    &s.track_number.unwrap_or(0),
//                    &s.artist.opt_str(),
//                    &s.title.opt_str(),
//                ));
//            }
//
//            tt_str
//        })
//        .collect();
//
//    options.extend(values.iter().map(|s| s.as_str()));
//
//    let i = input_options_loop(&msg, &options);
//
//    match i {
//        0 => None,
//        1 => Some(0),
//        2 => loop {
//            let new_value = input_loop_parse::<u16>("enter a new value:");
//            let msg = format!("new value: '{}'", new_value);
//
//            let i = input_options_loop(&msg, &["ok", "reenter value", "dismiss"]);
//
//            match i {
//                0 => return Some(new_value),
//                1 => continue,
//                _ => return None,
//            }
//        },
//        _ => return total_discs[i - 3].1,
//    }
//}

fn string_input(str: &str) -> String {
    loop {
        println!("{}", str);
        let mut input = String::new();

        match std::io::stdin().read_line(&mut input) {
            Ok(_) => {
                input.pop();
                return input;
            }
            Err(e) => println!("error:\n {}", e),
        }
    }
}

//fn input_loop_parse<T: FromStr + Default>(str: &str) -> T {
//    input_loop(str, |v| v.parse::<T>().is_ok()).parse::<T>().unwrap_or_else(|_| unreachable!())
//    // Can't use unwrap because FromStr::Err does not neccesarily implement Debug
//}

fn options_input(str: &str, options: &[&str]) -> usize {
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
            Ok(_) => match input.trim_matches('\n').parse::<usize>() {
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

fn confirm_input(str: &str) -> bool {
    loop {
        print!("{str} [y/N]?");
        let mut input = String::with_capacity(2);

        let _ = std::io::stdout().flush().is_ok();

        if let Err(e) = std::io::stdin().read_line(&mut input) {
            println!("error:\n {e}");
        } else {
            input.retain(|c| c != '\r' && c != '\n');
            input.make_ascii_lowercase();

            if input.is_empty() || input == "n" {
                return false;
            } else if input == "y" {
                return true;
            } else {
                println!("invalid input");
            }
        }
    }
}

fn successfull_early_exit() {
    println!("exiting...");
    std::process::exit(0);
}
