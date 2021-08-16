use colored::Colorize;
use music_organizer::{
    Changes, Checks, Cleanup, FileOpType, MusicIndex, Song, SongOperation, TagUpdate, Value,
};
use std::io::Write;
use std::path::Path;
use std::process::exit;

use crate::args::Args;

mod args;

const VERBOSE: usize = 2;

static mut LAST_LEN: usize = 0;

fn main() {
    let Args {
        music_dir,
        output_dir,
        verbosity,
        op_type,
        assume_yes,
        dry_run,
        no_check,
        no_cleanup,
    } = args::parse_args();

    let (op_type_sim_pres, op_type_pres_prog, op_type_sim_past) = match op_type {
        FileOpType::Copy => ("copy", "copying", "copied"),
        FileOpType::Move => ("move", "moving", "moved"),
    };
    let (rename_sim_pres, rename_pres_prog, rename_sim_past) = ("rename", "renaming", "renamed");

    println!("╭────────────────────────────────────────────────────────────╮");
    println!("│ Indexing                                                   │");
    println!("╰────────────────────────────────────────────────────────────╯");
    let mut index = MusicIndex::from(music_dir.clone());

    let mut i = 1;
    index.read(&mut |p| {
        print_verbose(
            &format!("{} {}", (i + 1).to_string().blue(), strip_dir(p, &music_dir).green()),
            verbosity >= 2,
        );
        i += 1;
    });
    reset_print_verbose();
    println!();

    let checks = Checks::from(&index);
    if !no_check {
        println!("╭────────────────────────────────────────────────────────────╮");
        println!("│ Checking                                                   │");
        println!("╰────────────────────────────────────────────────────────────╯");

        //changes.check_inconsitent_release_artists(inconsitent_artists_dialog);
        //changes.check_inconsitent_albums(inconsitent_albums_dialog);
        //changes.check_inconsitent_total_tracks(inconsitent_total_tracks_dialog);
        //changes.check_inconsitent_total_discs(inconsitent_total_discs_dialog);
        println!();
    }

    let changes = Changes::generate(checks, &output_dir);

    if changes.dir_creations.is_empty() && changes.song_operations.is_empty() {
        println!("{}", "nothing to do".green());
    } else {
        println!("╭────────────────────────────────────────────────────────────╮");
        println!("│ Changes                                                    │");
        println!("╰────────────────────────────────────────────────────────────╯");
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
            if !changes.song_operations.is_empty() {
                println!("songs:");
                for (i, f) in changes.song_operations.iter().enumerate() {
                    println!(
                        "{} {}",
                        (i + 1).to_string().blue(),
                        format_song_op(
                            &music_dir,
                            &output_dir,
                            f,
                            op_type_sim_pres,
                            rename_sim_pres,
                            verbosity
                        )
                    );
                }
                println!();
            }
            if !changes.file_operations.is_empty() {
                println!("others:");
                for (i, f) in changes.file_operations.iter().enumerate() {
                    println!(
                        "{} {}",
                        (i + 1).to_string().blue(),
                        format_file_op(
                            &music_dir,
                            &output_dir,
                            f.old_path,
                            &f.new_path,
                            op_type_sim_pres,
                            rename_sim_pres,
                        )
                    );
                }
                println!();
            }
        }

        println!(
            "{} dirs will be created.\n{} files will be {}.",
            changes.dir_creations.len(),
            changes.song_operations.len() + changes.file_operations.len(),
            op_type_sim_past,
        );

        if !assume_yes {
            let ok = input_confirmation_loop("continue");

            if !ok {
                println!("exiting...");
                exit(0);
            }
        }

        if dry_run {
            println!("skip writing dryrun...");
        } else {
            println!("╭────────────────────────────────────────────────────────────╮");
            println!("│ Writing                                                    │");
            println!("╰────────────────────────────────────────────────────────────╯");
            let mut i = 1;
            changes.dir_creations(&mut |d, r| {
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
                            "{} {} creating dir {}:\n{}",
                            (i + 1).to_string().blue(),
                            "error".red(),
                            d.path.display(),
                            e.to_string().red()
                        );
                    }
                }

                i += 1;
            });
            reset_print_verbose();

            let mut i = 1;
            changes.song_operations(op_type, &mut |f, r| {
                match r {
                    Ok(_) => {
                        let s = format!(
                            "{} {}",
                            (i + 1).to_string().blue(),
                            format_song_op(
                                &music_dir,
                                &output_dir,
                                f,
                                op_type_sim_past,
                                rename_sim_past,
                                verbosity
                            )
                        );
                        print_verbose(&s, verbosity >= 2);
                    }
                    Err(e) => {
                        reset_print_verbose();
                        println!(
                            "{} {} {}:\n{}",
                            (i + 1).to_string().blue(),
                            "error".red(),
                            format_song_op(
                                &music_dir,
                                &output_dir,
                                f,
                                op_type_pres_prog,
                                rename_pres_prog,
                                VERBOSE
                            ),
                            e.to_string().red(),
                        );
                    }
                }

                i += 1;
            });
            reset_print_verbose();

            let mut i = 1;
            changes.file_operations(op_type, &mut |f, r| {
                match r {
                    Ok(_) => {
                        let s = format!(
                            "{} {}",
                            (i + 1).to_string().blue(),
                            format_file_op(
                                &music_dir,
                                &output_dir,
                                f.old_path,
                                &f.new_path,
                                op_type_sim_past,
                                rename_sim_past,
                            )
                        );
                        print_verbose(&s, verbosity >= 2);
                    }
                    Err(e) => {
                        reset_print_verbose();
                        println!(
                            "{} {} {}:\n{}",
                            (i + 1).to_string().blue(),
                            "error".red(),
                            format_file_op(
                                &music_dir,
                                &output_dir,
                                f.old_path,
                                &f.new_path,
                                op_type_pres_prog,
                                rename_pres_prog,
                            ),
                            e.to_string().red(),
                        );
                    }
                }

                i += 1;
            });
            reset_print_verbose();
        }
    }

    if !no_cleanup {
        println!("╭────────────────────────────────────────────────────────────╮");
        println!("│ Cleanup                                                    │");
        println!("╰────────────────────────────────────────────────────────────╯");
        let mut cleanup = Cleanup::from(music_dir.clone());
        let mut i = 1;
        cleanup.check(&mut |p| {
            print_verbose(
                &format!("{} {}", i.to_string().blue(), strip_dir(p, &music_dir).green()),
                verbosity == 2,
            );

            i += 1;
        });
        reset_print_verbose();
        println!();

        if cleanup.dir_deletions.is_empty() {
            println!("{}", "nothing to cleanup".green());
        } else {
            if verbosity >= 1 && !cleanup.dir_deletions.is_empty() {
                println!("dirs:");

                for (i, d) in cleanup.dir_deletions.iter().enumerate() {
                    println!(
                        "{} delete {}",
                        (i + 1).to_string().blue(),
                        strip_dir(&d.path, &music_dir).red(),
                    );
                }
                println!();
            }

            println!("{} dirs will be deleted.", cleanup.dir_deletions.len());

            if !assume_yes {
                let ok = input_confirmation_loop("continue");

                if !ok {
                    println!("exiting...");
                    exit(0);
                }
            }

            if dry_run {
                println!("skip cleaning up dryrun...");
            } else {
                cleanup.excecute();
            }
        }
    }

    println!("{}", "done".green());
}

//fn inconsitent_artists_dialog(
//    index: &MusicIndex,
//    a: &ReleaseArtists,
//    b: &ReleaseArtists,
//) -> Option<String> {
//    fn print(index: &MusicIndex, artist: &ReleaseArtists) {
//        println!("{}:", artist.name.as_str().yellow());
//        for (i, al) in artist.releases.iter().enumerate() {
//            if i == 10 {
//                println!("   {}", "...".green());
//                break;
//            }
//            println!("   {}:", al.name);
//            for (j, s) in al.songs.iter().map(|&si| &index.songs[si]).enumerate() {
//                if i >= 4 || j == 3 {
//                    println!("      {}", "...".green());
//                    break;
//                } else {
//                    println!(
//                        "      {:02} - {} - {}",
//                        s.track_number.unwrap_or(0),
//                        s.artist.opt_str(),
//                        s.title.opt_str()
//                    );
//                }
//            }
//        }
//    }
//    println!("These two artists are named similarly:");
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
//                _ => return None,
//            }
//        },
//        _ => unreachable!(),
//    }
//}
//
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

fn format_song_op(
    music_dir: &Path,
    output_dir: &Path,
    file_op: &SongOperation,
    op_type_str: &str,
    rename_str: &str,
    verbosity: usize,
) -> String {
    match (&file_op.new_path, &file_op.tag_update) {
        (Some(new_path), Some(tag_update)) => format!(
            "{}\n{}",
            format_file_op(
                music_dir,
                output_dir,
                &file_op.song.path,
                new_path,
                op_type_str,
                rename_str
            ),
            format_tag_update(file_op.song, tag_update, verbosity),
        ),
        (None, Some(tag_update)) => format_tag_update(file_op.song, tag_update, verbosity),
        (Some(new_path), None) => format_file_op(
            music_dir,
            output_dir,
            &file_op.song.path,
            new_path,
            op_type_str,
            rename_str,
        ),
        (None, None) => String::new(),
    }
}

fn format_file_op(
    music_dir: &Path,
    output_dir: &Path,
    old_path: &Path,
    new_path: &Path,
    op_type_str: &str,
    rename_str: &str,
) -> String {
    let old = strip_dir(old_path, music_dir).yellow();

    let mut just_rename = false;
    let release_dir = old_path.parent().unwrap();
    let new = match new_path.strip_prefix(release_dir).ok() {
        Some(p) => {
            if p.components().count() == 1 {
                just_rename = true;
                p.display().to_string().green()
            } else {
                strip_dir(new_path, output_dir).green()
            }
        }
        None => strip_dir(new_path, output_dir).green(),
    };

    let operation = if just_rename { rename_str } else { op_type_str };
    if operation.len() + old.len() + new.len() + 5 <= 180 {
        format!("{} {} to {}", operation, old, new)
    } else {
        format!("{} {}\n    to {}", operation, old, new)
    }
}

fn format_tag_update(s: &Song, u: &TagUpdate, _verbosity: usize) -> String {
    let mut string = String::new();

    if let Some(s) = format_string_vec("release artists", &s.release_artists, &u.release_artists) {
        string.push_str(&s);
    }
    if let Some(s) = format_string_vec("artists", &s.artists, &u.artists) {
        string.push_str(&s);
    }
    if let Some(s) = format_string("release", &s.release, &u.release) {
        string.push_str(&s);
    }
    if let Some(s) = format_string("title", &s.title, &u.title) {
        string.push_str(&s);
    }
    if let Some(s) = format_u16("track number", s.track_number, u.track_number) {
        string.push_str(&s);
    }
    if let Some(s) = format_u16("total tracks", s.total_tracks, u.total_tracks) {
        string.push_str(&s);
    }
    if let Some(s) = format_u16("disc number", s.disc_number, u.track_number) {
        string.push_str(&s);
    }
    if let Some(s) = format_u16("total discs", s.total_discs, u.total_discs) {
        string.push_str(&s);
    }

    string
}

fn format_u16(name: &str, old: Option<u16>, new: Value<u16>) -> Option<String> {
    match (old, new) {
        (Some(old), Value::Update(new)) => Some(format!(
            "change {}: {} to {}",
            name,
            old.to_string().yellow(),
            new.to_string().green()
        )),
        (None, Value::Update(new)) => Some(format!("add {}: {}", name, new.to_string().green())),
        (Some(old), Value::Remove) => Some(format!("remove {}: {}", name, old.to_string().red())),
        _ => None,
    }
}

fn format_string(name: &str, old: &str, new: &Value<String>) -> Option<String> {
    match new {
        Value::Update(new) => Some(format!("change {}: {} to {}", name, old.yellow(), new.green())),
        Value::Remove => Some(format!("remove {}: {}", name, old.red())),
        Value::Unchanged => None,
    }
}

fn format_string_vec(name: &str, old: &[String], new: &Value<Vec<String>>) -> Option<String> {
    match new {
        Value::Update(new) => Some(format!(
            "change {}: {} to {}",
            name,
            old.join(", ").yellow(),
            new.join(", ").green()
        )),
        Value::Remove => Some(format!("remove {}: {}", name, old.join(", ").red())),
        Value::Unchanged => None,
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

//fn input_loop(str: &str, predicate: fn(&str) -> bool) -> String {
//    loop {
//        println!("{}", str);
//        let mut input = String::new();
//
//        match std::io::stdin().read_line(&mut input) {
//            Ok(_) => {
//                input.pop();
//                if predicate(&input) {
//                    return input;
//                }
//            }
//            Err(e) => println!("error:\n {}", e),
//        }
//    }
//}
//
//fn input_loop_parse<T: FromStr + Default>(str: &str) -> T {
//    input_loop(str, |v| v.parse::<T>().is_ok()).parse::<T>().unwrap_or_else(|_| unreachable!())
//    // Can't use unwrap because FromStr::Err does not neccesarily implement Debug
//}
//
//fn input_options_loop(str: &str, options: &[&str]) -> usize {
//    loop {
//        if !str.is_empty() {
//            println!("{}", str);
//        }
//        let mut input = String::with_capacity(2);
//
//        for (i, s) in options.iter().enumerate() {
//            if options.len() < 10 {
//                println!("[{}] {}", i, s.replace("\n", "\n    "));
//            } else {
//                println!("[{:02}] {}", i, s.replace("\n", "\n     "));
//            }
//        }
//
//        match std::io::stdin().read_line(&mut input) {
//            Ok(_) => match usize::from_str(input.trim_matches('\n')) {
//                Ok(i) => {
//                    if i < options.len() {
//                        return i;
//                    } else {
//                        println!("invalid input")
//                    }
//                }
//                Err(_) => println!("invalid input"),
//            },
//            Err(e) => println!("error:\n {}", e),
//        }
//    }
//}

fn input_confirmation_loop(str: &str) -> bool {
    loop {
        print!("{} [y/N]?", str);
        let mut input = String::with_capacity(2);

        let _ = std::io::stdout().flush().is_ok();

        if let Err(e) = std::io::stdin().read_line(&mut input) {
            println!("error:\n {}", e);
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

fn strip_dir(path: &Path, dir: &Path) -> String {
    path.strip_prefix(dir).unwrap().display().to_string()
}
