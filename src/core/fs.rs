use std::ffi::OsStr;
use std::fs;
use std::io;
use std::path::PathBuf;
use std::{error, path::Path};

use regex::Regex;

use crate::update::TagUpdate;
use crate::Song;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct DirCreation {
    pub path: PathBuf,
}

impl DirCreation {
    pub fn execute(&self) -> Result<(), io::Error> {
        std::fs::create_dir(&self.path)
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct DirDeletion {
    pub path: PathBuf,
}

impl DirDeletion {
    pub fn execute(&self) -> Result<(), io::Error> {
        std::fs::remove_dir(&self.path)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct SongOperation<'a> {
    pub song: &'a Song,
    pub tag_update: Option<TagUpdate>,
    pub new_path: Option<PathBuf>,
}

impl SongOperation<'_> {
    pub fn execute(&self, op_type: FileOpType) -> Result<(), Box<dyn error::Error>> {
        if let Some(new) = &self.new_path {
            match op_type {
                FileOpType::Copy => {
                    fs::copy(&self.song.path, new)?;
                }
                FileOpType::Move => {
                    fs::rename(&self.song.path, new)?;
                }
            };
        }

        if let Some(u) = &self.tag_update {
            match &self.new_path {
                Some(n) => u.execute(n)?,
                None => u.execute(&self.song.path)?,
            }
        }

        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct FileOperation<'a> {
    pub old_path: &'a Path,
    pub new_path: PathBuf,
}

impl FileOperation<'_> {
    pub fn execute(&self, op_type: FileOpType) -> Result<(), Box<dyn error::Error>> {
        match op_type {
            FileOpType::Copy => {
                fs::copy(&self.old_path, &self.new_path)?;
            }
            FileOpType::Move => {
                fs::rename(&self.old_path, &self.new_path)?;
            }
        };
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
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
    static ref RE: Regex = Regex::new(r#"[<>:"/\|?*]"#).unwrap();
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

const MUSIC_FILE_EXTENSIONS: [&str; 2] = ["m4a", "mp3"];

#[inline]
pub fn is_music_extension(s: &OsStr) -> bool {
    for e in &MUSIC_FILE_EXTENSIONS {
        if s.eq(*e) {
            return true;
        }
    }

    false
}

const IMAGE_FILE_EXTENSIONS: [&str; 3] = ["png", "jpg", "jpeg"];
#[inline]
pub fn is_image_extension(s: &OsStr) -> bool {
    for e in &IMAGE_FILE_EXTENSIONS {
        if s.eq(*e) {
            return true;
        }
    }

    false
}
