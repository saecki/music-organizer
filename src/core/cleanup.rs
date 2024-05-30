use std::path::{Path, PathBuf};

use crate::fs::DirDeletion;

fn is_empty_dir(cleanup: &mut Cleanup, dir: &Path, f: &mut impl FnMut(&Path)) -> bool {
    if dir.is_file() {
        return false;
    };

    f(dir);

    if let Ok(r) = std::fs::read_dir(dir) {
        let is_empty = r
            .into_iter()
            .filter_map(|e| e.ok())
            .map(|e| is_empty_dir(cleanup, &e.path(), f))
            .reduce(|a, b| a && b)
            .unwrap_or(true);

        if is_empty {
            cleanup.dir_deletions.push(DirDeletion { path: dir.to_owned() });
            return true;
        }
    }

    false
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Cleanup {
    pub dir_deletions: Vec<DirDeletion>,
    pub music_dir: PathBuf,
}

impl From<PathBuf> for Cleanup {
    fn from(music_dir: PathBuf) -> Self {
        Self { music_dir, ..Default::default() }
    }
}

impl Cleanup {
    pub fn check(&mut self, f: &mut impl FnMut(&Path)) {
        let dir = self.music_dir.to_owned();

        if let Ok(r) = std::fs::read_dir(dir) {
            for e in r.into_iter().filter_map(|e| e.ok()) {
                is_empty_dir(self, &e.path(), f);
            }
        }
    }

    pub fn excecute(&self, f: &mut impl FnMut(&Path)) {
        for d in &self.dir_deletions {
            std::fs::remove_dir(&d.path).ok();
            f(&d.path);
        }
    }

    pub fn is_empty(&self) -> bool {
        self.dir_deletions.is_empty()
    }
}
