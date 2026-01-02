use clap::{CommandFactory, Parser};
use music_organizer::{Changes, Checks, Cleanup, FileOpType, MusicIndex, ReleaseArtists, Value};
use std::fmt::Write as _;
use std::io::Write as _;
use std::path::Path;
use std::time::{Duration, Instant};

use crate::args::{
    Command, CompletionsCommand, EmbeddedArtworks, OrganizeCommand, TranscodeCommand,
};
use crate::display::strip_dir;
use crate::display::{
    ANSII_BLUE, ANSII_CLEAR, ANSII_CYAN_ON_BLACK, ANSII_GRAY, ANSII_GREEN, ANSII_GREEN_ON_BLACK,
    ANSII_PURPLE_ON_BLACK, ANSII_RED, ANSII_YELLOW, ANSII_YELLOW_ON_BLACK,
};

mod args;
mod display;

const VERBOSE: u8 = 2;
const MAX_TITLE_WITH: usize = 11;
const TITLE_INDEXING: &str = "INDEXING";
const TITLE_CHECKING: &str = "CHECKING";
const TITLE_CHANGES: &str = "CHANGES";
const TITLE_WRITING: &str = "WRITING";
const TITLE_CLEANUP: &str = "CLEANUP";
const TITLE_DELETIONS: &str = "DELETIONS";
const TITLE_CLEANING: &str = "CLEANING";
const TITLE_TRANSCODING: &str = "TRANSCODING";

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
    println!("{ANSII_PURPLE_ON_BLACK} {title}{:padding$}{ANSII_CLEAR} ", "");
}

fn print_subtitle(title: &str) {
    let padding = MAX_SUBTITLE_WITH - title.len() + 1;
    println!("{ANSII_CYAN_ON_BLACK} {title}{:padding$}{ANSII_CLEAR} ", "");
}

macro_rules! print_verbose {
    ($verbose:expr, $title:expr, $pat:expr, $($args:expr),*) => {{
        if $verbose {
            println!($pat $(,$args)*);
        } else {
            print!("\x1b[2K\r");
            let padding = MAX_TITLE_WITH - $title.len() + 1;
            print!("{ANSII_PURPLE_ON_BLACK} {}{:padding$}{ANSII_CLEAR} ", $title, "");
            print!($pat $(,$args)*);
            std::io::stdout().flush().ok();
        }
    }}
}

struct Timer {
    start: Instant,
    timings: Vec<(Instant, String)>,
}

impl Timer {
    fn new() -> Self {
        Self { start: Instant::now(), timings: Vec::new() }
    }

    fn time(&mut self, name: impl Into<String>) {
        self.timings.push((Instant::now(), name.into()));
    }

    fn display(&self) {
        println!();
        fn print_duration_aligned(duration: Duration) {
            let millis = duration.as_millis();
            let seconds = millis / 1000;
            let minutes = seconds / 60;
            if millis < 1000 {
                print!("{millis:6} {ANSII_GREEN}ms{ANSII_CLEAR}");
            } else if seconds < 60 {
                let seconds = millis as f64 / 1000.0;
                print!("{seconds:6.2} {ANSII_YELLOW}s{ANSII_CLEAR} ");
            } else {
                let seconds = seconds % 60;
                print!("{minutes:3}:{seconds:02}   ");
            }
        }

        let mut prev = self.start;
        for (time, name) in self.timings.iter() {
            print_duration_aligned(time.duration_since(prev));
            println!("  {name}");
            prev = *time;
        }

        println!("{ANSII_GRAY}---------------------{ANSII_CLEAR}");
        print_duration_aligned(prev.duration_since(self.start));
        println!();
    }
}

fn main() {
    let args = args::Args::parse();

    match args.command {
        Command::Organize(command) => organize(command),
        Command::Transcode(command) => transcode(command),
        Command::Completions(command) => completions(command),
    }
}

fn completions(args: CompletionsCommand) {
    let mut stdout = std::io::stdout();
    let mut app = args::Args::command();
    clap_complete::generate(args.shell, &mut app, "music-organizer", &mut stdout)
}

fn transcode(args: TranscodeCommand) {
    let mut timer = Timer::new();

    // indexing
    let mut index = MusicIndex::new(&args.music_dir);
    display_indexing(&mut index, args.verbosity);
    timer.time("indexing");

    // transcode
    display_transcoding(&index, &args.output_dir, args.verbosity);
    timer.time("transcode");

    // FIXME: Clean this up.
    for image_path in index.images.iter() {
        let sub_path = image_path
            .strip_prefix(index.music_dir)
            .expect("All songs should be located inside the `music-dir`");
        let new_path = args.output_dir.join(sub_path);

        if new_path.exists() {
            continue;
        }

        if let Some(parent) = new_path.parent() {
            _ = std::fs::create_dir_all(parent);
        }

        _ = std::fs::copy(image_path, new_path);
    }

    if args.timings {
        timer.display();
    }
}

fn display_transcoding(index: &MusicIndex, output_dir: &Path, verbosity: u8) {
    let verbose = verbosity >= 2;
    print_title_verbose(verbose, TITLE_TRANSCODING);

    let mut i = 0;
    let mut num_transcoded = 0;
    let mut num_copied = 0;
    music_organizer::transcode_songs(index, output_dir, &mut |op, res| {
        i += 1;
        match op.format {
            Some(_) => num_transcoded += 1,
            None => num_copied += 1,
        }

        let new_path = op.new_path.display();
        let (simp_past, pres_prog) = match op.format {
            Some(_) => ("transcoded", "transcoding"),
            None => ("copied", "copying"),
        };

        match &res {
            Ok(()) => {
                print_verbose!(
                    verbose,
                    TITLE_TRANSCODING,
                    "{ANSII_BLUE}{i}{ANSII_CLEAR} {simp_past} {ANSII_YELLOW}{new_path}{ANSII_YELLOW}",
                );
            }
            Err(err) => {
                print_verbose!(
                    false,
                    TITLE_TRANSCODING,
                    "{ANSII_BLUE}{i}{ANSII_CLEAR} {ANSII_RED}error{ANSII_CLEAR} \
                     {pres_prog} {ANSII_YELLOW}{new_path}{ANSII_YELLOW}: \
                     {ANSII_RED}{err:#}{ANSII_CLEAR}\n",
                );
            }
        }
    });

    if !verbose {
        print_verbose!(
            verbose,
            TITLE_TRANSCODING,
            "{ANSII_BLUE}{num_transcoded} {ANSII_GREEN}transcoded {ANSII_BLUE}{num_copied} {ANSII_GREEN}copied{ANSII_CLEAR}",
        );
    }

    println!();
}

fn organize(args: OrganizeCommand) {
    let dict = Dict {
        op_type: match args.op_type() {
            FileOpType::Move => MOVE_TENSES,
            FileOpType::Copy => COPY_TENSES,
        },
        rename: RENAME_TENSES,
    };

    let mut timer = Timer::new();

    // indexing
    let mut index = MusicIndex::new(&args.music_dir);
    display_indexing(&mut index, args.verbosity);
    timer.time("indexing");

    // checking
    let mut checks = Checks::from(&index);
    if !args.no_check {
        display_checking(&mut checks, &args);
    }
    timer.time("checking");

    // changes
    let changes = Changes::generate(checks, args.output_dir());
    display_changes(&changes, &args, &dict);
    timer.time("changes");

    if !changes.is_empty() {
        // writing
        if !args.assume_yes && !args.dry_run {
            let ok = confirm_input("continue");
            if !ok {
                successfull_early_exit();
            }
        }
        display_writing(&changes, &args, &dict);
        timer.time("writing");
    }

    if !args.no_cleanup {
        // cleanup
        let mut cleanup = Cleanup::from(args.music_dir.clone());
        display_cleanup(&mut cleanup, &args);
        timer.time("cleanup");

        // deletions
        display_deletions(&cleanup, &args);
        timer.time("deletions");

        if !cleanup.is_empty() {
            // cleaning
            if !args.assume_yes && !args.dry_run {
                let ok = confirm_input("continue");
                if !ok {
                    successfull_early_exit();
                }
            }
            display_cleaning(&cleanup, &args);
            timer.time("cleaning");
        }
    }

    if args.timings {
        timer.display();
    }
}

fn display_indexing(index: &mut MusicIndex, verbosity: u8) {
    let music_dir = index.music_dir;
    let verbose = verbosity >= 2;
    print_title_verbose(verbose, TITLE_INDEXING);

    let mut i = 0;
    index.read(&mut |item| {
        i += 1;

        let path = strip_dir(item.path(), music_dir);
        print_verbose!(
            verbose,
            TITLE_INDEXING,
            "{ANSII_BLUE}{i} {ANSII_YELLOW}{path}{ANSII_CLEAR}",
        );
    });
    if !verbose {
        print_verbose!(
            verbose,
            TITLE_INDEXING,
            "{ANSII_BLUE}{i} {ANSII_GREEN}files indexed{ANSII_CLEAR}",
        );
    }
    println!();
}

fn display_checking(checks: &mut Checks, args: &OrganizeCommand) {
    let verbose = args.verbosity >= 2;
    print_title_verbose(verbose, TITLE_CHECKING);

    if args.embedded_artworks != EmbeddedArtworks::Keep {
        print_verbose!(verbose, TITLE_CHECKING, "{ANSII_YELLOW}embedded artworks{ANSII_CLEAR}",);
        let only_redundant = args.embedded_artworks == EmbeddedArtworks::RemoveRedundant;
        checks.remove_embedded_artworks(only_redundant);
    }

    print_verbose!(verbose, TITLE_CHECKING, "{ANSII_YELLOW}file permissions{ANSII_CLEAR}",);
    checks.check_file_permissions();

    print_verbose!(verbose, TITLE_CHECKING, "{ANSII_YELLOW}inconsistent artists{ANSII_CLEAR}",);
    checks.check_inconsitent_release_artists(inconsitent_artists_dialog);
    //changes.check_inconsitent_albums(inconsitent_albums_dialog);
    //changes.check_inconsitent_total_tracks(inconsitent_total_tracks_dialog);
    //changes.check_inconsitent_total_discs(inconsitent_total_discs_dialog);

    if !verbose {
        print_verbose!(verbose, TITLE_CHECKING, "{ANSII_GREEN}done{ANSII_CLEAR}",);
    }

    println!();
}

fn display_changes(changes: &Changes, args: &OrganizeCommand, dict: &Dict) {
    if changes.is_empty() {
        let verbose = args.verbosity >= 2;
        print_title_verbose(verbose, TITLE_CHANGES);
        print_verbose!(verbose, TITLE_CHANGES, "{ANSII_GREEN}nothing to do{ANSII_CLEAR}\n",);
        return;
    }

    let verbose = args.verbosity >= 1;
    print_title_verbose(verbose, TITLE_CHANGES);

    if verbose {
        if !changes.dir_creations.is_empty() {
            print_subtitle(SUBTITLE_DIRS);
            for (i, d) in changes.dir_creations.iter().enumerate() {
                let n = i + 1;
                println!(
                    "{ANSII_BLUE}{n}{ANSII_CLEAR} create {ANSII_YELLOW}{}{ANSII_CLEAR}",
                    d.path.display()
                );
            }
            println!();
        }
        if !changes.song_operations.is_empty() {
            print_subtitle(SUBTITLE_SONGS);
            for (i, o) in changes.song_operations.values().enumerate() {
                let n = i + 1;
                println!(
                    "{ANSII_BLUE}{n}{ANSII_CLEAR} {}",
                    display::SongOp(
                        &args.music_dir,
                        args.output_dir(),
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
                let n = i + 1;
                println!(
                    "{ANSII_BLUE}{n}{ANSII_CLEAR} {}",
                    display::FileOp(
                        &args.music_dir,
                        args.output_dir(),
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
        "{ANSII_BLUE}{num_dir_creations}{ANSII_CLEAR} {} will be created{}{ANSII_BLUE}{num_file_ops}{ANSII_CLEAR} {} will be {}",
        if num_dir_creations == 1 { "dir" } else { "dirs" },
        if verbose { '\n' } else { ' ' },
        if num_file_ops == 1 { "file" } else { "files" },
        dict.op_type.sim_past
    );

    println!();
}

fn display_writing(changes: &Changes, args: &OrganizeCommand, dict: &Dict) {
    if args.dry_run {
        println!("skip writing dryrun...");
        return;
    }

    let verbose = args.verbosity >= 2;
    print_title_verbose(verbose, TITLE_WRITING);

    let mut dir_creation_idx = 0;
    changes.execute_dir_creations(&mut |d, r| {
        dir_creation_idx += 1;
        match r {
            Ok(_) => {
                print_verbose!(
                    verbose,
                    TITLE_WRITING,
                    "{ANSII_BLUE}{dir_creation_idx}{ANSII_CLEAR} created dir {}",
                    d.path.display()
                );
            }
            Err(e) => {
                print_verbose!(
                    false,
                    TITLE_WRITING,
                    "{ANSII_BLUE}{dir_creation_idx}{ANSII_CLEAR} {ANSII_RED}error{ANSII_CLEAR} creating dir {}: {ANSII_RED}{e}{ANSII_CLEAR}\n",
                    d.path.display()
                );
            }
        }
    });

    let mut file_operation_idx = 0;
    changes.execute_song_operations(args.op_type(), &mut |o, r| {
        file_operation_idx += 1;
        match r {
            Ok(_) => {
                print_verbose!(
                    verbose,
                    TITLE_WRITING,
                    "{ANSII_BLUE}{file_operation_idx}{ANSII_CLEAR} {}",
                    display::SongOp(
                        &args.music_dir,
                        args.output_dir(),
                        o,
                        dict.op_type.sim_past,
                        dict.rename.sim_past,
                        args.verbosity,
                    )
                );
            }
            Err(e) => {
                println!(
                    "{ANSII_BLUE}{file_operation_idx}{ANSII_CLEAR} {ANSII_RED}error{ANSII_CLEAR} {}:\n{ANSII_RED}{e}{ANSII_CLEAR}",
                    display::SongOp(
                        &args.music_dir,
                        args.output_dir(),
                        o,
                        dict.op_type.pres_prog,
                        dict.rename.pres_prog,
                        VERBOSE
                    ),
                );
            }
        }
    });

    changes.execute_file_operations(args.op_type(), &mut |f, r| {
        file_operation_idx += 1;
        match r {
            Ok(_) => {
                print_verbose!(
                    verbose,
                    TITLE_WRITING,
                    "{ANSII_BLUE}{file_operation_idx}{ANSII_CLEAR} {}",
                    display::FileOp(
                        &args.music_dir,
                        args.output_dir(),
                        f.old_path,
                        &f.new_path,
                        dict.op_type.sim_past,
                        dict.rename.sim_past,
                    )
                );
            }
            Err(e) => {
                println!(
                    "{ANSII_BLUE}{file_operation_idx}{ANSII_CLEAR} {ANSII_RED}error{ANSII_CLEAR} {}:\n{ANSII_RED}{e}{ANSII_CLEAR}",
                    display::FileOp(
                        &args.music_dir,
                        args.output_dir(),
                        f.old_path,
                        &f.new_path,
                        dict.op_type.pres_prog,
                        dict.rename.pres_prog,
                    )
                );
            }
        }
    });

    if !verbose {
        let num_dir_creations = dir_creation_idx;
        let num_file_ops = file_operation_idx;
        print_verbose!(
            verbose,
            TITLE_WRITING,
            "{ANSII_BLUE}{num_dir_creations} {ANSII_GREEN}{} {ANSII_BLUE}{num_file_ops} {ANSII_GREEN}{} {ANSII_GREEN}{}{ANSII_CLEAR}",
            if num_dir_creations == 1 { "dir created" } else { "dirs created" },
            if num_file_ops == 1 { "file" } else { "files" },
            dict.op_type.sim_past
        );
    }

    println!();
}

fn display_cleanup(cleanup: &mut Cleanup, args: &OrganizeCommand) {
    let verbose = args.verbosity >= 2;
    print_title_verbose(verbose, TITLE_CLEANUP);

    let mut i = 0;
    cleanup.check(&mut |p| {
        i += 1;
        print_verbose!(
            verbose,
            TITLE_CLEANUP,
            "{ANSII_BLUE}{i} {ANSII_YELLOW}{}{ANSII_CLEAR}",
            strip_dir(p, &args.music_dir)
        );
    });

    if !verbose {
        print_verbose!(
            verbose,
            TITLE_CLEANUP,
            "{ANSII_BLUE}{i} {ANSII_GREEN}dirs checked{ANSII_CLEAR}",
        );
    }

    println!();
}

fn display_deletions(cleanup: &Cleanup, args: &OrganizeCommand) {
    if cleanup.is_empty() {
        let verbose = args.verbosity >= 2;
        print_title_verbose(verbose, TITLE_DELETIONS);
        print_verbose!(verbose, TITLE_DELETIONS, "{ANSII_GREEN}nothing to cleanup{ANSII_CLEAR}\n",);
    } else {
        let verbose = args.verbosity >= 1;
        print_title_verbose(verbose, TITLE_DELETIONS);

        if verbose {
            print_subtitle(SUBTITLE_DIRS);

            for (i, d) in cleanup.dir_deletions.iter().enumerate() {
                let n = i + 1;
                println!(
                    "{ANSII_BLUE}{n}{ANSII_CLEAR} delete {ANSII_RED}{}{ANSII_RED}",
                    strip_dir(&d.path, &args.music_dir)
                );
            }
            println!();
        }

        let num_dir_deletions = cleanup.dir_deletions.len();
        print_verbose!(
            verbose,
            TITLE_DELETIONS,
            "{ANSII_BLUE}{num_dir_deletions}{ANSII_CLEAR} {} will be deleted",
            if num_dir_deletions == 1 { "dir" } else { "dirs" }
        );

        println!();
    }
}

fn display_cleaning(cleanup: &Cleanup, args: &OrganizeCommand) {
    if args.dry_run {
        println!("skip cleaning up dryrun...");
    } else {
        let verbose = args.verbosity >= 2;
        print_title_verbose(verbose, TITLE_CLEANING);

        let mut i = 0;
        cleanup.excecute(&mut |p| {
            i += 1;
            print_verbose!(
                verbose,
                TITLE_CLEANING,
                "{ANSII_BLUE}{i}{ANSII_CLEAR} deleted {ANSII_RED}{}{ANSII_CLEAR}",
                strip_dir(p, &args.music_dir)
            );
        });

        if !verbose {
            print_verbose!(
                verbose,
                TITLE_CLEANING,
                "{ANSII_BLUE}{i} {ANSII_GREEN}{}{ANSII_CLEAR}",
                if i == 1 { "dir deleted" } else { "dirs deleted" }
            );
        }
        println!();
    }
}

fn inconsitent_artists_dialog(a: &ReleaseArtists, b: &ReleaseArtists) -> Value<Vec<String>> {
    fn print(artist: &ReleaseArtists) {
        for n in artist.names {
            println!(" {ANSII_YELLOW_ON_BLACK}{n}{ANSII_CLEAR}");
        }
        println!();
        for (i, al) in artist.releases.iter().enumerate() {
            if i == 10 {
                println!("   {ANSII_GREEN}...{ANSII_CLEAR}");
                break;
            }
            println!("   {}:", al.name);
            for (j, s) in al.songs.iter().enumerate() {
                if i >= 4 || j == 3 {
                    println!("      {ANSII_GREEN}...{ANSII_CLEAR}");
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
                    _ = write!(msg, " {ANSII_GREEN_ON_BLACK}{n}{ANSII_CLEAR}");
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
