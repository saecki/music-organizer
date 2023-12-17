use std::ffi::OsStr;
use std::fs;
use std::io;
use std::path::PathBuf;
use std::{error, path::Path};

use regex::Regex;

use crate::meta::Mode;
use crate::update::TagUpdate;
use crate::Song;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DirCreation {
    pub path: PathBuf,
}

impl DirCreation {
    pub fn execute(&self) -> Result<(), io::Error> {
        std::fs::create_dir(&self.path)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DirDeletion {
    pub path: PathBuf,
}

impl DirDeletion {
    pub fn execute(&self) -> Result<(), io::Error> {
        std::fs::remove_dir(&self.path)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SongOperation<'a> {
    pub song: &'a Song,
    pub tag_update: Option<TagUpdate>,
    pub mode_update: Option<Mode>,
    pub new_path: Option<PathBuf>,
}

impl<'a> SongOperation<'a> {
    pub fn new(song: &'a Song) -> Self {
        Self { song, mode_update: None, tag_update: None, new_path: None }
    }

    pub fn execute(&self, op_type: FileOpType) -> Result<(), Box<dyn error::Error>> {
        let path = match &self.new_path {
            Some(new) => {
                match op_type {
                    FileOpType::Copy => {
                        fs::copy(&self.song.path, new)?;
                    }
                    FileOpType::Move => {
                        fs::rename(&self.song.path, new)?;
                    }
                }
                new
            }
            None => &self.song.path,
        };

        if let Some(u) = &self.tag_update {
            u.execute(path)?;
        }

        if let Some(mode) = &self.mode_update {
            mode.write(path)?;
        }

        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileOperation<'a> {
    pub old_path: &'a Path,
    pub new_path: PathBuf,
}

impl FileOperation<'_> {
    pub fn execute(&self, op_type: FileOpType) -> Result<(), Box<dyn error::Error>> {
        match op_type {
            FileOpType::Copy => {
                fs::copy(self.old_path, &self.new_path)?;
            }
            FileOpType::Move => {
                fs::rename(self.old_path, &self.new_path)?;
            }
        };
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileOpType {
    Move,
    Copy,
}

impl From<bool> for FileOpType {
    fn from(copy: bool) -> Self {
        match copy {
            true => Self::Copy,
            false => Self::Move,
        }
    }
}

lazy_static::lazy_static! {
    static ref RE: Regex = Regex::new(r#"[<>:"/\\|?*]"#).unwrap();
}

#[inline]
pub fn valid_os_str_dots(str: &str) -> String {
    let mut s = RE.replace_all(str, "").to_string();

    if s.starts_with('.') {
        // This is safe because we know that the first byte has to be present and is character of 1 byte length.
        unsafe {
            s.as_bytes_mut()[0] = b'_';
        }
    }
    if s.ends_with('.') {
        s.pop();
        s.push('_');
    }

    s
}

#[inline]
pub fn valid_os_str(str: &str) -> String {
    RE.replace_all(str, "").trim().to_string()
}

const SONG_EXTENSIONS: [&str; 3] = ["m4a", "mp3", "flac"];
#[inline]
pub fn is_song_extension(s: &OsStr) -> bool {
    for e in &SONG_EXTENSIONS {
        if s.eq(*e) {
            return true;
        }
    }

    false
}

const IMAGE_EXTENSIONS: [&str; 3] = ["png", "jpg", "jpeg"];
#[inline]
pub fn is_image_extension(s: &OsStr) -> bool {
    for e in &IMAGE_EXTENSIONS {
        if s.eq(*e) {
            return true;
        }
    }

    false
}
