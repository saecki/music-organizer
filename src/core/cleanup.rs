use std::fs::ReadDir;
use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicUsize;

use crossbeam_channel::Sender;

use crate::DeleteDirOp;
use crate::thread::{Msg, WorkerState, worker_pool};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Cleanup<'a> {
    pub root: &'a Path,
    pub dir_deletions: Vec<DeleteDirOp>,
}

impl<'a> Cleanup<'a> {
    pub fn new(music_dir: &'a Path) -> Self {
        Self { root: music_dir, dir_deletions: Vec::new() }
    }
}

impl Cleanup<'_> {
    pub fn check(&mut self, f: &mut impl FnMut(&Path)) {
        let idx = AtomicUsize::new(0);
        let (item_sender, item_receiver) = crossbeam_channel::unbounded();

        let dir_path = self.root.to_owned();
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

        // Never delete the music root directory.
        self.dir_deletions.extend(dirs.into_iter().skip(1).filter_map(|dir| {
            let dir = dir?;
            dir.empty.then_some(DeleteDirOp { path: dir.path })
        }));
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
