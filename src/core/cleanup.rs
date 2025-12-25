use std::path::{Path, PathBuf};

use crate::fs::DirDeletion;

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
        let dir_path = self.music_dir.to_owned();

        if let Ok(dir_iter) = std::fs::read_dir(&dir_path) {
            let mut dir_stack = vec![(dir_path, dir_iter, true)];
            check_empty_dir(self, &mut dir_stack, f);
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

fn check_empty_dir(
    cleanup: &mut Cleanup,
    dir_stack: &mut Vec<(PathBuf, std::fs::ReadDir, bool)>,
    f: &mut impl FnMut(&Path),
) {
    // traverse depth first to be more memory efficient
    'stack: while let Some((_, dir_iter, is_empty)) = dir_stack.last_mut() {
        while let Some(entry) = dir_iter.next() {
            let Ok(entry) = entry else {
                *is_empty = false;
                continue;
            };

            let Ok(ft) = entry.file_type() else { continue };
            if !ft.is_dir() {
                *is_empty = false;
                continue;
            }

            let dir_path = entry.path();
            f(&dir_path);

            let Ok(dir_iter) = std::fs::read_dir(&dir_path) else {
                *is_empty = false;
                continue;
            };

            // traverse sub-directory before continuing here
            dir_stack.push((dir_path, dir_iter, true));
            continue 'stack;
        }

        let (dir_path, _, is_empty) = dir_stack.pop().unwrap();
        if is_empty {
            cleanup.dir_deletions.push(DirDeletion { path: dir_path });
        } else {
            // mark parent directory as non-empty if this directory is non-empty
            if let Some((_, _, parent_is_empty)) = dir_stack.last_mut() {
                *parent_is_empty = false;
            }
        }
    }
}
