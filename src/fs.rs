use std::error;
use std::ffi::OsStr;
use std::fs;
use std::io;
use std::path::PathBuf;

use regex::Regex;

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

#[inline]
pub fn is_music_extension(s: &OsStr) -> bool {
    for e in &MUSIC_FILE_EXTENSIONS {
        if s.eq(*e) {
            return true;
        }
    }

    false
}
