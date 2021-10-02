use colored::Colorize;
use music_organizer::{Changes, Checks, Cleanup, FileOpType, MusicIndex};
use std::io::Write;
use std::process::exit;

use crate::args::Args;
use crate::display::strip_dir;

mod args;
mod display;

const VERBOSE: usize = 2;

macro_rules! print_verbose {
    ($verbose:expr, $pat:expr, $($args:expr),*) => {{
        if $verbose {
            println!($pat $(,$args)*);
        } else {
            print!("\x1b[2K\r");
            print!($pat $(,$args)*);
            let _ = std::io::stdout().flush().is_ok();
        }
    }}
}

fn main() {
    let Args {
        music_dir,
        output_dir,
        verbosity,
        op_type,
        assume_yes,
        dry_run,
        no_check,
        keep_embedded_artworks,
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
        print_verbose!(
            verbosity >= 2,
            "{} {}",
            (i + 1).to_string().blue(),
            strip_dir(p, &music_dir).green()
        );
        i += 1;
    });
    println!();

    let mut checks = Checks::from(&index);
    if !no_check {
        println!("╭────────────────────────────────────────────────────────────╮");
        println!("│ Checking                                                   │");
        println!("╰────────────────────────────────────────────────────────────╯");

        if !keep_embedded_artworks {
            checks.remove_embedded_artworks();
        }

        //changes.check_inconsitent_release_artists(inconsitent_artists_dialog);
        //changes.check_inconsitent_albums(inconsitent_albums_dialog);
        //changes.check_inconsitent_total_tracks(inconsitent_total_tracks_dialog);
        //changes.check_inconsitent_total_discs(inconsitent_total_discs_dialog);
        println!();
    }

    let changes = Changes::generate(checks, &output_dir);

    if changes.is_empty() {
        println!("{}\n", "nothing to do".green());
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
                for (i, o) in changes.song_operations.iter().enumerate() {
                    println!(
                        "{} {}",
                        (i + 1).to_string().blue(),
                        display::SongOp(
                            &music_dir,
                            &output_dir,
                            o,
                            op_type_sim_pres,
                            rename_sim_pres,
                            verbosity,
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
                        display::FileOp(
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
                        print_verbose!(
                            verbosity >= 2,
                            "{} created dir {}",
                            (i + 1).to_string().blue(),
                            d.path.display()
                        );
                    }
                    Err(e) => {
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

            let mut i = 1;
            changes.song_operations(op_type, &mut |o, r| {
                match r {
                    Ok(_) => {
                        print_verbose!(
                            verbosity >= 2,
                            "{} {}",
                            (i + 1).to_string().blue(),
                            display::SongOp(
                                &music_dir,
                                &output_dir,
                                o,
                                op_type_sim_past,
                                rename_sim_past,
                                verbosity,
                            )
                        );
                    }
                    Err(e) => {
                        println!(
                            "{} {} {}:\n{}",
                            (i + 1).to_string().blue(),
                            "error".red(),
                            display::SongOp(
                                &music_dir,
                                &output_dir,
                                o,
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

            let mut i = 1;
            changes.file_operations(op_type, &mut |f, r| {
                match r {
                    Ok(_) => {
                        print_verbose!(
                            verbosity >= 2,
                            "{} {}",
                            (i + 1).to_string().blue(),
                            display::FileOp(
                                &music_dir,
                                &output_dir,
                                f.old_path,
                                &f.new_path,
                                op_type_sim_past,
                                rename_sim_past,
                            )
                        );
                    }
                    Err(e) => {
                        print!(
                            "{} {} {}:\n{}",
                            (i + 1).to_string().blue(),
                            "error".red(),
                            display::FileOp(
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
        }
        println!();
    }

    if !no_cleanup {
        println!("╭────────────────────────────────────────────────────────────╮");
        println!("│ Cleanup                                                    │");
        println!("╰────────────────────────────────────────────────────────────╯");
        let mut cleanup = Cleanup::from(music_dir.clone());
        let mut i = 1;
        cleanup.check(&mut |p| {
            print_verbose!(
                verbosity == 2,
                "{} {}",
                i.to_string().blue(),
                strip_dir(p, &music_dir).green()
            );

            i += 1;
        });
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
                let mut i = 1;
                cleanup.excecute(&mut |p| {
                    print_verbose!(
                        verbosity >= 1,
                        "{} {} {}",
                        i.to_string().blue(),
                        "delete".red(),
                        strip_dir(p, &music_dir).red()
                    );
                    i += 1;
                });
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
