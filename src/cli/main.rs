use clap::{CommandFactory, Parser};
use indexmap::IndexMap;
use music_organizer::{
    Artists, Checks, Cleanup, CopyFileOp, CreateDirOp, DeleteDirOp, DeleteFileOp, FileOp, Item,
    MusicIndex, OrganizeChanges, Song, SongOp, TranscodeChanges, TranscodeOp, Value,
};
use std::fmt::Write as _;
use std::io::Write as _;
use std::path::Path;
use std::time::{Duration, Instant};

use crate::args::{
    Command, CompletionsCommand, EmbeddedArtworks, OrganizeCommand, TranscodeCommand,
};
use crate::display::{
    ANSII_BLUE, ANSII_CLEAR, ANSII_CYAN_ON_BLACK, ANSII_GRAY, ANSII_GREEN, ANSII_GREEN_ON_BLACK,
    ANSII_PURPLE_ON_BLACK, ANSII_RED, ANSII_YELLOW, ANSII_YELLOW_ON_BLACK,
};
use crate::display::{Tense, strip_dir};

mod args;
mod display;

const MAX_TITLE_WIDTH: usize = 11;
const TITLE_INDEXING: &str = "INDEXING";
const TITLE_CHECKING: &str = "CHECKING";
const TITLE_CHANGES: &str = "CHANGES";
const TITLE_WRITING: &str = "WRITING";
const TITLE_CLEANUP: &str = "CLEANUP";
const TITLE_DELETIONS: &str = "DELETIONS";
const TITLE_CLEANING: &str = "CLEANING";
const TITLE_TRANSCODING: &str = "TRANSCODING";

const MAX_SUBTITLE_WIDTH: usize = 9;
const SUBTITLE_DIRS: &str = "dirs";
const SUBTITLE_SONGS: &str = "songs";
const SUBTITLE_OTHERS: &str = "others";
const SUBTITLE_TRANSCODE: &str = "transcode";
const SUBTITLE_COPY: &str = "copy";
const SUBTITLE_DELETE: &str = "delete";

fn print_title_verbose(verbose: bool, title: &str) {
    if verbose {
        print_title(title)
    }
}

fn print_title(title: &str) {
    println!("{ANSII_PURPLE_ON_BLACK} {title:<MAX_TITLE_WIDTH$} {ANSII_CLEAR} ");
}

fn print_subtitle(title: &str) {
    println!("{ANSII_CYAN_ON_BLACK} {title:<MAX_SUBTITLE_WIDTH$} {ANSII_CLEAR} ");
}

macro_rules! print_verbose {
    ($verbose:expr, $title:expr, $($pat:literal),+ $(,)?) => {{
        if $verbose {
            $(print!($pat);)+
            println!();
        } else {
            print!("\x1b[2K\r");
            let title = $title;
            print!("{ANSII_PURPLE_ON_BLACK} {title:MAX_TITLE_WIDTH$} {ANSII_CLEAR} ");
            $(print!($pat);)+
            std::io::stdout().flush().ok();
        }
    }}
}

macro_rules! print_op {
    ($verbose:expr, $title:expr, $n:expr, $($pat:literal),+ $(,)?) => {{
        let n = $n;
        print_verbose!($verbose, $title, "{ANSII_BLUE}{n}{ANSII_CLEAR} ", $($pat),+);
    }}
}

macro_rules! print_failed_op {
    ($title:expr, $n:expr, $($pat:literal,)+ $err:ident $(,)?) => {{
        let err = $err;
        print_op!(
            false, $title, $n,
            "{ANSII_RED}error{ANSII_CLEAR} ",
            $($pat,)+
            ": {ANSII_RED}{err:#}{ANSII_CLEAR}\n",
        );
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
    if args.music_dir == args.output_dir {
        let output_dir = args.output_dir.display();
        println!(
            "{ANSII_RED}error:{ANSII_CLEAR} invalid value '{ANSII_YELLOW}{output_dir}{ANSII_CLEAR}' \
             for '--output-dir <OUTPUT_DIR>': `music-dir` and `output-dir` must be different directories"
        );
        std::process::exit(1);
    }

    let mut timer = Timer::new();

    // indexing
    let mut index_a = MusicIndex::new(&args.music_dir);
    display_indexing(&mut index_a, args.verbosity, false);
    let mut index_b = MusicIndex::new(&args.output_dir);
    display_indexing(&mut index_b, args.verbosity, false);
    timer.time("indexing");

    // changes
    let changes = TranscodeChanges::generate(&index_a, &index_b);
    display_transcode_changes(&changes, &args);
    timer.time("changes");

    if !changes.is_empty() {
        if !args.assume_yes && !args.dry_run {
            let ok = confirm_input("continue");
            timer.time("confirm");
            if !ok {
                successfull_early_exit();
            }
        }

        // transcode
        display_transcoding(&changes, &args);
        timer.time("transcode");
    }

    if !args.no_cleanup {
        // cleanup
        let mut cleanup = Cleanup::new(&args.output_dir);
        display_cleanup(&mut cleanup, &args.output_dir, args.verbosity);
        timer.time("cleanup");

        if !cleanup.is_empty() {
            // deletions
            display_deletions(&cleanup, args.verbosity);
            timer.time("deletions");

            if !args.assume_yes && !args.dry_run {
                let ok = confirm_input("continue");
                timer.time("confirm");
                if !ok {
                    successfull_early_exit();
                }
            }

            // cleaning
            display_cleaning(&cleanup, args.dry_run, args.verbosity);
            timer.time("cleaning");
        }
    }

    if args.timings {
        timer.display();
    }
}

fn display_transcode_changes(changes: &TranscodeChanges, args: &TranscodeCommand) {
    if changes.is_empty() {
        let verbose = args.verbosity >= 2;
        print_title_verbose(verbose, TITLE_CHANGES);
        print_verbose!(verbose, TITLE_CHANGES, "{ANSII_GREEN}nothing to do{ANSII_CLEAR}\n",);
        return;
    }

    let verbose = args.verbosity >= 1;
    print_title_verbose(verbose, TITLE_CHANGES);

    if verbose {
        if changes.dir_creations().next().is_some() {
            print_subtitle(SUBTITLE_DIRS);
            for (dc, n) in changes.dir_creations().zip(1..) {
                let path = strip_dir(&dc.path, &args.output_dir);
                println!("{ANSII_BLUE}{n}{ANSII_CLEAR} create {ANSII_GREEN}{path}{ANSII_CLEAR}",);
            }
            println!();
        }
        if !changes.transcode_ops.is_empty() {
            print_subtitle(SUBTITLE_TRANSCODE);
            for (op, n) in changes.transcode_ops.iter().zip(1..) {
                let op = display::transcode_op(&args.music_dir, op, Tense::SimPres);
                println!("{ANSII_BLUE}{n}{ANSII_CLEAR} {op}");
            }
            println!();
        }
        if !changes.copy_ops.is_empty() {
            print_subtitle(SUBTITLE_COPY);
            for (op, n) in changes.copy_ops.iter().zip(1..) {
                let op =
                    display::copy_file_op(&args.music_dir, &args.output_dir, op, Tense::SimPres);
                println!("{ANSII_BLUE}{n}{ANSII_CLEAR} {op}",);
            }
            println!();
        }
        if !changes.delete_ops.is_empty() {
            print_subtitle(SUBTITLE_DELETE);
            for (op, n) in changes.delete_ops.iter().zip(1..) {
                let path = op.path.display();
                println!("{ANSII_BLUE}{n}{ANSII_CLEAR} delete {ANSII_RED}{path}{ANSII_CLEAR}",);
            }
            println!();
        }
    }

    let num_dir_creations = changes.dir_creations().count();
    let num_transcoded = changes.transcode_ops.len();
    let num_copied = changes.copy_ops.len();
    let num_deleted = changes.delete_ops.len();

    let dir_or_dirs = if num_dir_creations == 1 { "dir" } else { "dirs" };
    let song_or_songs = if num_dir_creations == 1 { "song" } else { "songs" };
    let copied_file_or_files = if num_dir_creations == 1 { "file" } else { "files" };
    let deleted_file_or_files = if num_dir_creations == 1 { "file" } else { "files" };

    let sep = if verbose { '\n' } else { ' ' };
    print_verbose!(
        verbose,
        TITLE_TRANSCODING,
        "{ANSII_BLUE}{num_dir_creations}{ANSII_CLEAR} {dir_or_dirs} will be {ANSII_GREEN}created{sep}\
         {ANSII_BLUE}{num_transcoded}{ANSII_CLEAR} {song_or_songs} will be {ANSII_GREEN}transcoded{sep}\
         {ANSII_BLUE}{num_copied}{ANSII_CLEAR} {copied_file_or_files} will be {ANSII_GREEN}copied{sep}\
         {ANSII_BLUE}{num_deleted}{ANSII_CLEAR} {deleted_file_or_files} will be {ANSII_RED}deleted{ANSII_CLEAR}",
    );

    println!();
}

fn display_transcoding(changes: &TranscodeChanges, args: &TranscodeCommand) {
    if args.dry_run {
        println!("skip transcoding dryrun...");
        return;
    }

    let verbose = args.verbosity >= 2;
    print_title_verbose(verbose, TITLE_TRANSCODING);

    display_create_dir_ops(TITLE_TRANSCODING, changes.dir_creations(), verbose);
    #[rustfmt::skip]
    display_transcode_operations(TITLE_TRANSCODING, changes.a.root, &changes.transcode_ops, verbose);
    #[rustfmt::skip]
    display_copy_file_ops(TITLE_TRANSCODING, changes.a.root, changes.b.root, &changes.copy_ops, verbose);
    display_delete_file_ops(TITLE_TRANSCODING, &changes.delete_ops, verbose);

    if !verbose {
        let num_dir_creations = changes.dir_creations().count();
        let num_transcoded = changes.transcode_ops.len();
        let num_copied = changes.copy_ops.len();
        let num_deleted = changes.delete_ops.len();

        let dir_or_dirs = if num_dir_creations == 1 { "dir" } else { "dirs" };
        let song_or_songs = if num_dir_creations == 1 { "song" } else { "songs" };
        let copied_file_or_files = if num_dir_creations == 1 { "file" } else { "files" };
        let deleted_file_or_files = if num_dir_creations == 1 { "file" } else { "files" };

        let sep = if verbose { '\n' } else { ' ' };
        print_verbose!(
            verbose,
            TITLE_TRANSCODING,
            "{ANSII_BLUE}{num_dir_creations}{ANSII_CLEAR} {dir_or_dirs} {ANSII_GREEN}created{sep}\
             {ANSII_BLUE}{num_transcoded}{ANSII_CLEAR} {song_or_songs} {ANSII_GREEN}transcoded{sep}\
             {ANSII_BLUE}{num_copied}{ANSII_CLEAR} {copied_file_or_files} {ANSII_GREEN}copied{sep}\
             {ANSII_BLUE}{num_deleted}{ANSII_CLEAR} {deleted_file_or_files} {ANSII_RED}deleted{ANSII_CLEAR}",
        );
    }

    println!();
}

fn organize(args: OrganizeCommand) {
    let mut timer = Timer::new();

    // indexing
    let mut index = MusicIndex::new(&args.music_dir);
    display_indexing(&mut index, args.verbosity, args.show_others);
    timer.time("indexing");

    // checking
    let mut checks = Checks::new(&index);
    if !args.no_check {
        display_checking(&mut checks, &args);
    }
    timer.time("checking");

    // changes
    let changes = OrganizeChanges::generate(checks);
    display_organize_changes(&changes, &args);
    timer.time("changes");

    if !changes.is_empty() {
        if !args.assume_yes && !args.dry_run {
            let ok = confirm_input("continue");
            timer.time("confirm");
            if !ok {
                successfull_early_exit();
            }
        }

        // writing
        display_writing(&changes, &args);
        timer.time("writing");
    }

    if !args.no_cleanup {
        // cleanup
        let mut cleanup = Cleanup::new(&args.music_dir);
        display_cleanup(&mut cleanup, &args.music_dir, args.verbosity);
        timer.time("cleanup");

        if !cleanup.is_empty() {
            // deletions
            display_deletions(&cleanup, args.verbosity);
            timer.time("deletions");

            if !args.assume_yes && !args.dry_run {
                let ok = confirm_input("continue");
                timer.time("confirm");
                if !ok {
                    successfull_early_exit();
                }
            }

            // cleaning
            display_cleaning(&cleanup, args.dry_run, args.verbosity);
            timer.time("cleaning");
        }
    }

    if args.timings {
        timer.display();
    }
}

fn display_indexing(index: &mut MusicIndex, verbosity: u8, show_others: bool) {
    let music_dir = index.root;
    let verbose = verbosity >= 2;
    print_title_verbose(verbose, TITLE_INDEXING);

    let mut i = 0;
    index.read(&mut |item| {
        i += 1;

        let path = strip_dir(item.path(), music_dir);
        match item {
            Item::Error(_, err) => {
                print_failed_op!(TITLE_INDEXING, i, "{ANSII_YELLOW}{path}{ANSII_CLEAR}", err);
            }
            Item::Other(..) if show_others => {
                print_op!(false, TITLE_INDEXING, i, "other {ANSII_YELLOW}{path}{ANSII_CLEAR}\n");
            }
            _ => {
                print_op!(verbose, TITLE_INDEXING, i, "{ANSII_YELLOW}{path}{ANSII_CLEAR}");
            }
        };
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

fn display_organize_changes(changes: &OrganizeChanges, args: &OrganizeCommand) {
    if changes.is_empty() {
        let verbose = args.verbosity >= 2;
        print_title_verbose(verbose, TITLE_CHANGES);
        print_verbose!(verbose, TITLE_CHANGES, "{ANSII_GREEN}nothing to do{ANSII_CLEAR}\n",);
        return;
    }

    let verbose = args.verbosity >= 1;
    print_title_verbose(verbose, TITLE_CHANGES);

    if verbose {
        if changes.dir_creations().next().is_some() {
            print_subtitle(SUBTITLE_DIRS);
            for (dc, n) in changes.dir_creations().zip(1..) {
                let path = strip_dir(&dc.path, &args.music_dir);
                println!("{ANSII_BLUE}{n}{ANSII_CLEAR} create {ANSII_GREEN}{path}{ANSII_CLEAR}");
            }
            println!();
        }
        if !changes.song_ops.is_empty() {
            print_subtitle(SUBTITLE_SONGS);
            for (op, n) in changes.song_ops.values().zip(1..) {
                let op = display::song_op(&args.music_dir, op, Tense::SimPres);
                println!("{ANSII_BLUE}{n}{ANSII_CLEAR} {op}",);
            }
            println!();
        }
        if !changes.file_ops.is_empty() {
            print_subtitle(SUBTITLE_OTHERS);
            for (op, n) in changes.file_ops.values().zip(1..) {
                let op = display::file_op(&args.music_dir, op, Tense::SimPres);
                println!("{ANSII_BLUE}{n}{ANSII_CLEAR} {op}",);
            }
            println!();
        }
    }

    let num_dir_creations = changes.dir_creations().count();
    let num_song_ops = changes.song_ops.len();
    let num_file_ops = changes.file_ops.len();

    let dir_or_dirs = if num_dir_creations == 1 { "dir" } else { "dirs" };
    let song_or_songs = if num_file_ops == 1 { "song" } else { "songs" };
    let file_or_files = if num_file_ops == 1 { "file" } else { "files" };

    let sep = if verbose { '\n' } else { ' ' };
    print_verbose!(
        verbose,
        TITLE_CHANGES,
        "{ANSII_BLUE}{num_dir_creations}{ANSII_CLEAR} {dir_or_dirs} will be {ANSII_GREEN}created{sep}\
         {ANSII_BLUE}{num_song_ops}{ANSII_CLEAR} {song_or_songs} will be {ANSII_GREEN}moved{sep}\
         {ANSII_BLUE}{num_file_ops}{ANSII_CLEAR} {file_or_files} will be {ANSII_GREEN}moved{ANSII_CLEAR}",
    );

    println!();
}

fn display_writing(changes: &OrganizeChanges, args: &OrganizeCommand) {
    if args.dry_run {
        println!("skip writing dryrun...");
        return;
    }

    let verbose = args.verbosity >= 2;
    print_title_verbose(verbose, TITLE_WRITING);

    display_create_dir_ops(TITLE_WRITING, changes.dir_creations(), verbose);
    display_song_operations(TITLE_WRITING, changes.index.root, &changes.song_ops, verbose);
    display_file_operations(TITLE_WRITING, changes.index.root, &changes.file_ops, verbose);

    if !verbose {
        // TODO: This isn't quite accurate when song ops contain mode updates, or tag updates.
        let num_dir_creations = changes.dir_creations().count();
        let num_song_ops = changes.song_ops.len();
        let num_file_ops = changes.file_ops.len();

        let dir_or_dirs = if num_dir_creations == 1 { "dir" } else { "dirs" };
        let song_or_songs = if num_file_ops == 1 { "song" } else { "songs" };
        let file_or_files = if num_file_ops == 1 { "file" } else { "files" };

        let sep = if verbose { '\n' } else { ' ' };
        print_verbose!(
            verbose,
            TITLE_CHANGES,
            "{ANSII_BLUE}{num_dir_creations}{ANSII_CLEAR} {dir_or_dirs} {ANSII_GREEN}created{sep}\
             {ANSII_BLUE}{num_song_ops}{ANSII_CLEAR} {song_or_songs} {ANSII_GREEN}moved{sep}\
             {ANSII_BLUE}{num_file_ops}{ANSII_CLEAR} {file_or_files} {ANSII_GREEN}moved{ANSII_CLEAR}",
        );
    }

    println!();
}

fn display_cleanup(cleanup: &mut Cleanup, root: &Path, verbosity: u8) {
    let verbose = verbosity >= 2;
    print_title_verbose(verbose, TITLE_CLEANUP);

    let mut i = 0;
    cleanup.check(&mut |path| {
        i += 1;

        let path = strip_dir(path, root);
        print_op!(verbose, TITLE_CLEANUP, i, "{ANSII_YELLOW}{path}{ANSII_CLEAR}",);
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

fn display_deletions(cleanup: &Cleanup, verbosity: u8) {
    if cleanup.is_empty() {
        let verbose = verbosity >= 2;
        print_title_verbose(verbose, TITLE_DELETIONS);
        print_verbose!(verbose, TITLE_DELETIONS, "{ANSII_GREEN}nothing to cleanup{ANSII_CLEAR}\n",);

        return;
    }

    let verbose = verbosity >= 1;
    print_title_verbose(verbose, TITLE_DELETIONS);

    if verbose {
        print_subtitle(SUBTITLE_DIRS);

        for (d, n) in cleanup.dir_deletions.iter().zip(1..) {
            let path = d.path.display();
            println!("{ANSII_BLUE}{n}{ANSII_CLEAR} delete {ANSII_RED}{path}{ANSII_RED}");
        }
        println!();
    }

    let num_dir_deletions = cleanup.dir_deletions.len();
    let dir_or_dirs = if num_dir_deletions == 1 { "dir" } else { "dirs" };
    print_verbose!(
        verbose,
        TITLE_DELETIONS,
        "{ANSII_BLUE}{num_dir_deletions}{ANSII_CLEAR} {dir_or_dirs} will be {ANSII_RED}deleted{ANSII_CLEAR}",
    );

    println!();
}

fn display_cleaning(cleanup: &Cleanup, dry_run: bool, verbosity: u8) {
    if dry_run {
        println!("skip cleaning up dryrun...");
        return;
    }

    let verbose = verbosity >= 2;
    print_title_verbose(verbose, TITLE_CLEANING);

    display_delete_dir_ops(TITLE_CLEANING, &cleanup.dir_deletions, verbose);

    if !verbose {
        let num_dir_deletions = cleanup.dir_deletions.len();
        let dir_or_dirs = if num_dir_deletions == 1 { "dir" } else { "dirs" };
        print_verbose!(
            verbose,
            TITLE_CLEANING,
            "{ANSII_BLUE}{num_dir_deletions} {ANSII_RED}{dir_or_dirs} deleted{ANSII_CLEAR}",
        );
    }

    println!();
}

pub fn display_song_operations(
    title: &str,
    music_dir: &Path,
    song_ops: &IndexMap<*const Song, SongOp>,
    verbose: bool,
) {
    for (op, n) in song_ops.values().zip(1..) {
        let res = op.execute();
        let op = display::song_op(music_dir, op, Tense::from_result(&res));
        match res {
            Ok(()) => {
                print_op!(verbose, title, n, "{op}");
            }
            Err(err) => {
                print_failed_op!(title, n, "{op}", err);
            }
        }
    }
}

fn display_transcode_operations(
    title: &str,
    music_dir: &Path,
    transcode_ops: &[TranscodeOp],
    verbose: bool,
) {
    let mut i = 0;
    music_organizer::transcode_songs(transcode_ops, |op, res| {
        i += 1;

        let op = display::transcode_op(music_dir, op, Tense::from_result(&res));
        match res {
            Ok(()) => {
                print_op!(verbose, title, i, "{op}");
            }
            Err(err) => {
                print_failed_op!(title, i, "{op}", err);
            }
        }
    });
}

pub fn display_file_operations(
    title: &str,
    music_dir: &Path,
    file_ops: &IndexMap<*const Path, FileOp>,
    verbose: bool,
) {
    for (op, n) in file_ops.values().zip(1..) {
        let res = op.execute();
        let op = display::file_op(music_dir, op, Tense::from_result(&res));
        match res {
            Ok(_) => {
                print_op!(verbose, title, n, "{op}");
            }
            Err(err) => {
                print_failed_op!(title, n, "{op}", err);
            }
        }
    }
}

pub fn display_copy_file_ops(
    title: &str,
    music_dir: &Path,
    output_dir: &Path,
    file_ops: &[CopyFileOp],
    verbose: bool,
) {
    for (op, n) in file_ops.iter().zip(1..) {
        let res = op.execute();
        let op = display::copy_file_op(music_dir, output_dir, op, Tense::from_result(&res));
        match res {
            Ok(_) => {
                print_op!(verbose, title, n, "{op}");
            }
            Err(err) => {
                print_failed_op!(title, n, "{op}", err);
            }
        }
    }
}

pub fn display_create_dir_ops<'a>(
    title: &str,
    dir_creations: impl Iterator<Item = &'a CreateDirOp>,
    verbose: bool,
) {
    for (dc, n) in dir_creations.zip(1..) {
        let path = dc.path.display();
        match dc.execute() {
            Ok(()) => {
                print_op!(verbose, title, n, "created dir {ANSII_GREEN}{path}{ANSII_CLEAR}");
            }
            Err(err) => {
                print_failed_op!(title, n, "creating dir {ANSII_YELLOW}{path}{ANSII_CLEAR}", err);
            }
        }
    }
}

pub fn display_delete_dir_ops(title: &str, dir_deletions: &[DeleteDirOp], verbose: bool) {
    for (dc, n) in dir_deletions.iter().rev().zip(1..) {
        let path = dc.path.display();
        match dc.execute() {
            Ok(()) => {
                print_op!(verbose, title, n, "deleted dir {ANSII_RED}{path}{ANSII_CLEAR}");
            }
            Err(err) => {
                print_failed_op!(title, n, "deleting dir {ANSII_YELLOW}{path}{ANSII_CLEAR}", err);
            }
        }
    }
}

pub fn display_delete_file_ops(title: &str, delete_ops: &[DeleteFileOp], verbose: bool) {
    for (op, n) in delete_ops.iter().zip(1..) {
        let path = op.path.display();
        match op.execute() {
            Ok(()) => {
                print_op!(verbose, title, n, "deleted {ANSII_RED}{path}{ANSII_CLEAR}");
            }
            Err(err) => {
                print_failed_op!(title, n, "deleting {ANSII_YELLOW}{path}{ANSII_CLEAR}", err);
            }
        }
    }
}

fn inconsitent_artists_dialog(a: &Artists, b: &Artists) -> Value<Vec<String>> {
    fn print(artist: &Artists) {
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

fn options_input(str: &str, options: &[&str]) -> usize {
    loop {
        if !str.is_empty() {
            println!("{}", str);
        }
        let mut input = String::with_capacity(2);

        for (i, s) in options.iter().enumerate() {
            if options.len() < 10 {
                println!("[{i}] {}", s.replace("\n", "\n    "));
            } else {
                println!("[{i:02}] {}", s.replace("\n", "\n     "));
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
            Err(e) => println!("error: {}", e),
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
