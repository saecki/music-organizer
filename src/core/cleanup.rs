use std::fs::ReadDir;
use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicUsize;

use crossbeam_channel::Sender;

use crate::fs::DirDeletion;
use crate::thread::{Msg, WorkerState, worker_pool};

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
        let idx = AtomicUsize::new(0);
        let (item_sender, item_receiver) = crossbeam_channel::unbounded();

        let dir_path = self.music_dir.to_owned();
        let Ok(dir_iter) = std::fs::read_dir(&dir_path) else {
            return;
        };
        let work_dir = WorkDir::new(None, next_id(&idx), dir_path, dir_iter);
        let num_workers = 8;
        let mut dirs = Vec::new();
        worker_pool(
            num_workers,
            [work_dir],
            |_| CleanupBuilder::new(&idx, item_sender.clone()),
            || {
                while let Ok(Msg::Work((id, dir))) = item_receiver.recv() {
                    f(&dir.path);
                    let required_len = id.idx() + 1;
                    if dirs.len() < required_len {
                        dirs.resize(required_len, None);
                    }
                    dirs[id.idx()] = Some(dir);
                }
            },
        );

        // One linear scan to mark empty parent directories is enough, since
        // children directories always have higher IDs than parent directories.
        for i in (0..dirs.len()).rev() {
            let dir = dirs[i].as_ref().unwrap();
            let empty = dir.empty;
            if let Some(parent) = dir.parent {
                let parent = dirs[parent.idx()].as_mut().unwrap();
                parent.empty &= empty;
            }
        }

        for dir in dirs.into_iter() {
            let dir = dir.unwrap();
            if dir.empty {
                self.dir_deletions.push(DirDeletion { path: dir.path });
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

#[derive(Clone, Copy)]
struct DirId(NonZeroUsize);

impl DirId {
    fn idx(self) -> usize {
        self.0.get() - 1
    }
}

#[derive(Clone)]
struct Dir {
    parent: Option<DirId>,
    path: PathBuf,
    empty: bool,
}

struct WorkDir {
    parent: Option<DirId>,
    id: DirId,
    path: PathBuf,
    iter: ReadDir,
    empty: bool,
}

impl WorkDir {
    fn new(parent: Option<DirId>, id: DirId, path: PathBuf, iter: ReadDir) -> Self {
        Self { parent, id, path, iter, empty: true }
    }
}

struct CleanupBuilder<'a> {
    idx: &'a AtomicUsize,
    sender: Sender<Msg<(DirId, Dir)>>,
}

impl<'a> CleanupBuilder<'a> {
    fn new(idx: &'a AtomicUsize, sender: Sender<Msg<(DirId, Dir)>>) -> Self {
        Self { idx, sender }
    }
}

impl WorkerState<WorkDir> for CleanupBuilder<'_> {
    fn work(worker: &mut crate::thread::Worker<Self, WorkDir>, mut dir: WorkDir) {
        'dir: loop {
            for entry in &mut dir.iter {
                let Ok(entry) = entry else {
                    dir.empty = false;
                    continue;
                };

                let Ok(ft) = entry.file_type() else {
                    dir.empty = false;
                    continue;
                };
                if !ft.is_dir() {
                    dir.empty = false;
                    continue;
                }

                let dir_path = entry.path();

                let Ok(dir_iter) = std::fs::read_dir(&dir_path) else {
                    dir.empty = false;
                    continue;
                };

                // Traverse sub-directory and put parent back on the stack.
                let sub_dir =
                    WorkDir::new(Some(dir.id), next_id(worker.state.idx), dir_path, dir_iter);
                let parent_dir = std::mem::replace(&mut dir, sub_dir);
                worker.push_work(parent_dir);
                continue 'dir;
            }

            let finished = Dir { parent: dir.parent, path: dir.path, empty: dir.empty };
            worker.state.sender.send(Msg::Work((dir.id, finished))).unwrap();

            break;
        }
    }

    fn stop(&mut self) {
        self.sender.send(Msg::Stop).unwrap();
    }
}

fn next_id(idx: &AtomicUsize) -> DirId {
    let idx = idx.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    // SAFETY: We're adding one.
    DirId(unsafe { NonZeroUsize::new_unchecked(idx + 1) })
}
