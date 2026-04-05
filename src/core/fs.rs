use std::cell::LazyCell;
use std::ffi::OsStr;
use std::path::Path;
use std::path::PathBuf;

use regex::Regex;

use crate::Song;
use crate::meta::Mode;
use crate::update::TagUpdate;

#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct CreateDirOp {
    pub path: PathBuf,
}

impl CreateDirOp {
    pub fn execute(&self) -> Result<(), std::io::Error> {
        std::fs::create_dir(&self.path)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DeleteDirOp {
    pub path: PathBuf,
}

impl DeleteDirOp {
    pub fn execute(&self) -> Result<(), std::io::Error> {
        std::fs::remove_dir(&self.path)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SongOp<'a> {
    pub song: &'a Song,
    pub tag_update: Option<TagUpdate>,
    pub mode_update: Option<Mode>,
    pub new_path: Option<PathBuf>,
}

impl<'a> SongOp<'a> {
    pub fn new(song: &'a Song) -> Self {
        Self { song, mode_update: None, tag_update: None, new_path: None }
    }

    pub fn execute(&self) -> anyhow::Result<()> {
        let path = match &self.new_path {
            Some(new) => {
                std::fs::rename(&self.song.path, new)?;
                new
            }
            None => &self.song.path,
        };

        if let Some(update) = &self.tag_update {
            update.execute(path, self.song.format)?;
        }

        if let Some(mode) = &self.mode_update {
            mode.write(path)?;
        }

        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileOp<'a> {
    pub old_path: &'a Path,
    pub mode_update: Option<Mode>,
    pub new_path: Option<PathBuf>,
}

impl<'a> FileOp<'a> {
    pub fn new(file: &'a Path) -> Self {
        Self { old_path: file, mode_update: None, new_path: None }
    }

    pub fn execute(&self) -> anyhow::Result<()> {
        let path = match &self.new_path {
            Some(new) => {
                std::fs::rename(self.old_path, new)?;
                new
            }
            None => self.old_path,
        };

        if let Some(mode) = &self.mode_update {
            mode.write(path)?;
        }

        Ok(())
    }
}

/// Copy a file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CopyFileOp<'a> {
    pub old_path: &'a Path,
    pub new_path: PathBuf,
}

impl CopyFileOp<'_> {
    pub fn execute(&self) -> anyhow::Result<()> {
        std::fs::copy(self.old_path, &self.new_path)?;
        Ok(())
    }
}

/// Delete a file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeleteFileOp<'a> {
    pub path: &'a Path,
}

impl DeleteFileOp<'_> {
    pub fn execute(&self) -> anyhow::Result<()> {
        std::fs::remove_file(self.path)?;
        Ok(())
    }
}

thread_local! {
    static RE: LazyCell<Regex> = LazyCell::new(|| Regex::new(r#"[<>:"/\\|?*]"#).unwrap());
}

#[inline]
pub fn valid_os_str_dots(str: &str) -> String {
    let mut s = RE.with(|re| re.replace_all(str, "").to_string());

    if s.starts_with('.') {
        s.replace_range(0..1, "_");
    }
    if s.ends_with('.') {
        s.pop();
        s.push('_');
    }

    s
}

#[inline]
pub fn valid_os_str(str: &str) -> String {
    RE.with(|re| re.replace_all(str, "").trim().to_string())
}

#[inline]
pub fn fast_path_eq(l: &Path, r: &Path) -> bool {
    l.as_os_str() == r.as_os_str()
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
