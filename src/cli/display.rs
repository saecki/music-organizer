use std::fmt::Display;
use std::path::Path;

use music_organizer::{Song, SongOperation, TagUpdate, Value};

pub const ANSII_CLEAR: &str = "\x1b[0m";
pub const ANSII_RED: &str = "\x1b[31m";
pub const ANSII_GREEN: &str = "\x1b[32m";
pub const ANSII_YELLOW: &str = "\x1b[33m";
pub const ANSII_BLUE: &str = "\x1b[34m";

pub const ANSII_GREEN_ON_BLACK: &str = "\x1b[32;40m";
pub const ANSII_YELLOW_ON_BLACK: &str = "\x1b[33;40m";
pub const ANSII_PURPLE_ON_BLACK: &str = "\x1b[35;40m";
pub const ANSII_CYAN_ON_BLACK: &str = "\x1b[36;40m";

pub struct SongOp<'a>(
    pub &'a Path,
    pub &'a Path,
    pub &'a SongOperation<'a>,
    pub &'a str,
    pub &'a str,
    pub u8,
);

impl Display for SongOp<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        format_song_op(f, self.0, self.1, self.2, self.3, self.4, self.5)
    }
}

pub struct FileOp<'a>(
    pub &'a Path,
    pub &'a Path,
    pub &'a Path,
    pub &'a Path,
    pub &'a str,
    pub &'a str,
);

impl Display for FileOp<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        format_file_op(f, self.0, self.1, self.2, self.3, self.4, self.5)
    }
}

/// TODO: proper mode formatting
fn format_song_op(
    f: &mut impl std::fmt::Write,
    music_dir: &Path,
    output_dir: &Path,
    song_op: &SongOperation,
    op_type_str: &str,
    rename_str: &str,
    verbosity: u8,
) -> std::fmt::Result {
    if let Some(mode) = song_op.mode_update {
        println!("mode {mode} ");
    }
    match (&song_op.new_path, &song_op.tag_update) {
        (Some(new_path), Some(tag_update)) => {
            format_file_op(
                f,
                music_dir,
                output_dir,
                &song_op.song.path,
                new_path,
                op_type_str,
                rename_str,
            )?;
            f.write_char('\n')?;
            format_tag_update(f, song_op.song, tag_update, verbosity)
        }
        (None, Some(tag_update)) => {
            format_tag_update(f, song_op.song, tag_update, verbosity)?;
            write!(f, " {ANSII_GREEN}{}{ANSII_CLEAR}", strip_dir(&song_op.song.path, music_dir))
        }
        (Some(new_path), None) => format_file_op(
            f,
            music_dir,
            output_dir,
            &song_op.song.path,
            new_path,
            op_type_str,
            rename_str,
        ),
        (None, None) => Ok(()),
    }
}

fn format_file_op(
    f: &mut impl std::fmt::Write,
    music_dir: &Path,
    output_dir: &Path,
    old_path: &Path,
    new_path: &Path,
    op_type_str: &str,
    rename_str: &str,
) -> std::fmt::Result {
    let old = strip_dir(old_path, music_dir);

    let mut just_rename = false;
    let release_dir = old_path.parent().unwrap();
    let new = match new_path.strip_prefix(release_dir).ok() {
        Some(p) => {
            if p.components().count() == 1 {
                just_rename = true;
                p.display()
            } else {
                strip_dir(new_path, output_dir)
            }
        }
        None => strip_dir(new_path, output_dir),
    };

    let operation = if just_rename { rename_str } else { op_type_str };
    write!(f, "{operation} {ANSII_YELLOW}{old}{ANSII_CLEAR} to {ANSII_GREEN}{new}{ANSII_CLEAR}")?;

    Ok(())
}

/// TODO: prettier tag update
fn format_tag_update(
    f: &mut impl std::fmt::Write,
    s: &Song,
    u: &TagUpdate,
    _verbosity: u8,
) -> std::fmt::Result {
    format_string_vec(f, "release artists", &s.release_artists, &u.release_artists)?;
    format_string_vec(f, "artists", &s.artists, &u.artists)?;
    format_string(f, "release", &s.release, &u.release)?;
    format_string(f, "title", &s.title, &u.title)?;
    format_u16(f, "track number", s.track_number, u.track_number)?;
    format_u16(f, "total tracks", s.total_tracks, u.total_tracks)?;
    format_u16(f, "disc number", s.disc_number, u.track_number)?;
    format_u16(f, "total discs", s.total_discs, u.total_discs)?;
    format_value(f, "artwork", s.has_artwork, &u.artwork)?;

    Ok(())
}

fn format_u16(
    f: &mut impl std::fmt::Write,
    name: &str,
    old: Option<u16>,
    new: Value<u16>,
) -> Result<bool, std::fmt::Error> {
    match (old, new) {
        (Some(old), Value::Update(new)) => write!(
            f,
            "change {name}: {ANSII_YELLOW}{old}{ANSII_CLEAR} to {ANSII_GREEN}{new}{ANSII_CLEAR}"
        )?,
        (None, Value::Update(new)) => write!(f, "add {name}: {ANSII_GREEN}{new}{ANSII_CLEAR}")?,
        (Some(old), Value::Remove) => write!(f, "remove {name}: {ANSII_RED}{old}{ANSII_CLEAR}")?,
        _ => return Ok(false),
    }

    Ok(true)
}

fn format_string(
    f: &mut impl std::fmt::Write,
    name: &str,
    old: &str,
    new: &Value<String>,
) -> Result<bool, std::fmt::Error> {
    match new {
        Value::Update(new) => write!(
            f,
            "change {name}: {ANSII_YELLOW}{old}{ANSII_CLEAR} to {ANSII_GREEN}{new}{ANSII_CLEAR}"
        )?,
        Value::Remove => write!(f, "remove {name}: {ANSII_RED}{old}{ANSII_CLEAR}")?,
        Value::Unchanged => return Ok(false),
    }

    Ok(true)
}

fn format_string_vec(
    f: &mut impl std::fmt::Write,
    name: &str,
    old: &[String],
    new: &Value<Vec<String>>,
) -> Result<bool, std::fmt::Error> {
    match new {
        Value::Update(new) => write!(
            f,
            "change {name}: {ANSII_YELLOW}{}{ANSII_CLEAR} to {ANSII_GREEN}{}{ANSII_CLEAR}",
            old.join(", "),
            new.join(", ")
        )?,
        Value::Remove => write!(f, "remove {name}: {ANSII_RED}{}{ANSII_CLEAR}", old.join(", "))?,
        Value::Unchanged => return Ok(false),
    }

    Ok(true)
}

fn format_value<T>(
    f: &mut impl std::fmt::Write,
    name: &str,
    old: bool,
    new: &Value<T>,
) -> Result<bool, std::fmt::Error> {
    match (old, new) {
        (true, Value::Update(_)) => write!(f, "change {name}")?,
        (false, Value::Update(_)) => write!(f, "add {name}")?,
        (true, Value::Remove) => write!(f, "remove {name}")?,
        _ => return Ok(false),
    }

    Ok(true)
}

pub fn strip_dir<'a>(path: &'a Path, dir: &Path) -> std::path::Display<'a> {
    path.strip_prefix(dir).unwrap().display()
}
