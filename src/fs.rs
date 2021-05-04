use std::error;
use std::ffi::{OsStr, OsString};
use std::fs;
use std::io;
use std::path::PathBuf;

use crate::update::TagUpdate;

const MUSIC_FILE_EXTENSIONS: [&str; 5] = ["m4a", "mp3", "m4b", "m4p", "m4v"];

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
pub struct FileOperation {
    pub old: PathBuf,
    pub new: Option<PathBuf>,
    pub tag_update: Option<TagUpdate>,
}

impl FileOperation {
    pub fn execute(&self, op_type: FileOpType) -> Result<(), Box<dyn error::Error>> {
        if let Some(new) = &self.new {
            match op_type {
                FileOpType::Copy => fs::copy(&self.old, new).map(|_| ())?,
                FileOpType::Move => fs::rename(&self.old, new)?,
            };
        }

        //if let Some(u) = &self.tag_update {
        //    match &self.new {
        //        Some(n) => u.execute(n)?,
        //        None => u.execute(&self.old)?,
        //    }
        //}

        Ok(())
    }
}

#[derive(Copy, Clone, Debug, PartialEq)]
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
    static ref RE: regex::Regex = regex::Regex::new(r#"[<>:"/\|?*]"#).unwrap();
}

pub fn valid_os_string(str: &str) -> OsString {
    let mut s = RE.replace_all(str, "").to_string();

    if s.starts_with('.') {
        s.replace_range(0..1, "_");
    }

    if s.ends_with('.') {
        s.replace_range(s.len() - 1..s.len(), "_");
    }

    OsString::from(s)
}

#[inline]
pub fn is_music_extension(s: &OsStr) -> bool {
    for e in &MUSIC_FILE_EXTENSIONS {
        if s.eq(*e) {
            return true;
        }
    }

    false
}
