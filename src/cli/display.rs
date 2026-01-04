use std::fmt::Display;
use std::path::Path;

use music_organizer::{
    AudioFormat, CopyFileOp, MoveFileOp, Song, SongOp, TagUpdate, TranscodeOp, Value,
};

pub const ANSII_CLEAR: &str = "\x1b[0m";

pub const ANSII_RED: &str = "\x1b[31m";
pub const ANSII_GREEN: &str = "\x1b[32m";
pub const ANSII_YELLOW: &str = "\x1b[33m";
pub const ANSII_BLUE: &str = "\x1b[34m";

pub const ANSII_GRAY: &str = "\x1b[90m";

pub const ANSII_GREEN_ON_BLACK: &str = "\x1b[32;40m";
pub const ANSII_YELLOW_ON_BLACK: &str = "\x1b[33;40m";
pub const ANSII_PURPLE_ON_BLACK: &str = "\x1b[35;40m";
pub const ANSII_CYAN_ON_BLACK: &str = "\x1b[36;40m";

#[derive(Clone, Copy)]
pub enum Tense {
    /// Simple present, e.g. `copy`.
    SimPres,
    /// Simple past, e.g. `copied`.
    SimPast,
    /// Present progressive, e.g. `copying`.
    PresProg,
}

impl Tense {
    pub const fn from_result<T, E>(result: &Result<T, E>) -> Self {
        match result {
            Ok(_) => Self::SimPast,
            Err(_) => Self::PresProg,
        }
    }

    pub const fn move_(self) -> &'static str {
        use Tense::*;
        match self {
            SimPres => "move",
            SimPast => "moved",
            PresProg => "moving",
        }
    }

    pub const fn copy(self) -> &'static str {
        use Tense::*;
        match self {
            SimPres => "copy",
            SimPast => "copied",
            PresProg => "copying",
        }
    }

    pub const fn rename(self) -> &'static str {
        use Tense::*;
        match self {
            SimPres => "rename",
            SimPast => "renamed",
            PresProg => "renaming",
        }
    }

    pub const fn transcode(self) -> &'static str {
        use Tense::*;
        match self {
            SimPres => "transcode",
            SimPast => "transcoded",
            PresProg => "transcoding",
        }
    }
}

pub fn song_op(music_dir: &Path, op: &SongOp, tense: Tense) -> impl Display {
    display(move |f| format_song_op(f, music_dir, op, tense))
}

pub fn transcode_op(music_dir: &Path, op: &TranscodeOp, tense: Tense) -> impl Display {
    display(move |f| format_transcode_op(f, music_dir, op, tense))
}

pub fn move_file_op<'a>(
    music_dir: &'a Path,
    op: &'a MoveFileOp,
    tense: Tense,
) -> impl Display + use<'a> {
    display(move |f| format_move_file_op(f, music_dir, op.old_path, &op.new_path, tense))
}

pub fn copy_file_op<'a>(
    music_dir: &'a Path,
    output_dir: &'a Path,
    op: &'a CopyFileOp,
    tense: Tense,
) -> impl Display + use<'a> {
    display(move |f| {
        format_copy_file_op(f, music_dir, output_dir, op.old_path, &op.new_path, tense)
    })
}

/// TODO: proper mode formatting
fn format_song_op(
    f: &mut impl std::fmt::Write,
    music_dir: &Path,
    song_op: &music_organizer::SongOp,
    tense: Tense,
) -> std::fmt::Result {
    if let Some(mode) = song_op.mode_update {
        writeln!(f, "mode {mode} ")?;
    }
    match (&song_op.new_path, &song_op.tag_update) {
        (Some(new_path), Some(tag_update)) => {
            format_move_file_op(f, music_dir, &song_op.song.path, new_path, tense)?;
            f.write_char('\n')?;
            format_tag_update(f, song_op.song, tag_update)
        }
        (None, Some(tag_update)) => {
            format_tag_update(f, song_op.song, tag_update)?;
            write!(f, " {ANSII_GREEN}{}{ANSII_CLEAR}", strip_dir(&song_op.song.path, music_dir))
        }
        (Some(new_path), None) => {
            format_move_file_op(f, music_dir, &song_op.song.path, new_path, tense)
        }
        (None, None) => Ok(()),
    }
}

fn format_transcode_op(
    f: &mut impl std::fmt::Write,
    music_dir: &Path,
    op: &TranscodeOp,
    tense: Tense,
) -> std::fmt::Result {
    let operation = tense.transcode();
    let old = strip_dir(&op.song.path, music_dir);
    let format = AudioFormat::from(op.format);
    write!(
        f,
        "{operation} {ANSII_YELLOW}{old}{ANSII_CLEAR} to {ANSII_GREEN}{format}{ANSII_GREEN}"
    )?;
    Ok(())
}

fn format_move_file_op(
    f: &mut impl std::fmt::Write,
    music_dir: &Path,
    old_path: &Path,
    new_path: &Path,
    tense: Tense,
) -> std::fmt::Result {
    let old = strip_dir(old_path, music_dir);

    let just_rename = old_path.parent() == new_path.parent();
    let (operation, new) = if just_rename {
        (tense.rename(), Path::new(new_path.file_name().unwrap()).display())
    } else {
        (tense.move_(), strip_dir(music_dir, new_path))
    };
    write!(f, "{operation} {ANSII_YELLOW}{old}{ANSII_CLEAR} to {ANSII_GREEN}{new}{ANSII_CLEAR}")?;

    Ok(())
}

fn format_copy_file_op(
    f: &mut impl std::fmt::Write,
    music_dir: &Path,
    output_dir: &Path,
    old_path: &Path,
    new_path: &Path,
    tense: Tense,
) -> std::fmt::Result {
    let operation = tense.copy();
    let old_sub_path = old_path.strip_prefix(music_dir).unwrap();
    let new_sub_path = new_path.strip_prefix(output_dir).unwrap();
    write!(f, "{operation} {ANSII_YELLOW}{}{ANSII_CLEAR}", old_sub_path.display())?;
    if old_sub_path != new_sub_path {
        write!(f, " to {ANSII_GREEN}{}{ANSII_CLEAR}", new_sub_path.display())?;
    }

    Ok(())
}

/// TODO: prettier tag update
fn format_tag_update(f: &mut impl std::fmt::Write, s: &Song, u: &TagUpdate) -> std::fmt::Result {
    format_string_vec(f, "release artists", &s.album_artists, &u.release_artists)?;
    format_string_vec(f, "artists", &s.artists, &u.artists)?;
    format_string(f, "release", &s.album, &u.release)?;
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

#[track_caller]
pub fn strip_dir<'a>(path: &'a Path, dir: &Path) -> std::path::Display<'a> {
    path.strip_prefix(dir).unwrap().display()
}

fn display<F>(fmt: F) -> impl Display
where
    F: Fn(&mut std::fmt::Formatter) -> std::fmt::Result,
{
    struct Wrapper<F>(F);

    impl<F> Display for Wrapper<F>
    where
        F: Fn(&mut std::fmt::Formatter) -> std::fmt::Result,
    {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            (self.0)(f)
        }
    }

    Wrapper(fmt)
}
