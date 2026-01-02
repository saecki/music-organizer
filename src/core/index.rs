use crossbeam_channel::Sender;
use std::path::{Path, PathBuf};

use crate::fs::is_image_extension;
use crate::meta::IncompleteSong;
use crate::thread::{worker_pool, Msg, Worker, WorkerState};
use crate::{AudioFormat, Metadata, Song};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MusicIndex<'a> {
    pub music_dir: &'a Path,
    /// Songs that have necessary metadata.
    pub songs: Vec<Song>,
    /// Songs that are missing some necessary metadata.
    pub unknown_songs: Vec<IncompleteSong>,
    pub images: Vec<PathBuf>,
    pub other: Vec<PathBuf>,
}

impl<'a> MusicIndex<'a> {
    pub fn new(music_dir: &'a Path) -> Self {
        Self {
            music_dir,
            songs: Vec::new(),
            unknown_songs: Vec::new(),
            images: Vec::new(),
            other: Vec::new(),
        }
    }

    pub fn read(&mut self, f: &mut impl FnMut(&Item)) {
        let (item_sender, item_receiver) = crossbeam_channel::unbounded();

        // 8 threads seems to be the sweat spot on my machine :)
        let num_workers = 8;
        worker_pool(
            num_workers,
            [self.music_dir.to_path_buf()],
            |_| MusicIndexBuilder { item_sender: item_sender.clone() },
            || {
                while let Ok(Msg::Work(item)) = item_receiver.recv() {
                    f(&item);
                    match item {
                        Item::Song((path, format, meta)) => {
                            match Song::try_from((path, format, meta)) {
                                Ok(song) => self.songs.push(song),
                                Err(unknown_song) => self.unknown_songs.push(unknown_song),
                            }
                        }
                        Item::Image(path) => self.images.push(path),
                        Item::Other(path) => self.other.push(path),
                    }
                }
            },
        );
    }
}

pub enum Item {
    Song((PathBuf, AudioFormat, Metadata)),
    Image(PathBuf),
    Other(PathBuf),
}

impl Item {
    pub fn path(&self) -> &Path {
        match self {
            Self::Song((path, ..)) => path,
            Self::Image(path) => path,
            Self::Other(path) => path,
        }
    }
}

struct MusicIndexBuilder {
    item_sender: Sender<Msg<Item>>,
}

impl WorkerState<PathBuf> for MusicIndexBuilder {
    fn work(worker: &mut Worker<Self, PathBuf>, dir: PathBuf) {
        let Ok(r) = std::fs::read_dir(dir) else {
            return;
        };

        // Read subdirectory
        for e in r.into_iter().filter_map(|e| e.ok()) {
            let p = e.path();

            if p.is_file() {
                worker.state.add_item(p);
            } else if p.is_dir() {
                worker.push_work(p);
            }
        }
    }

    fn stop(&mut self) {
        self.item_sender.send(Msg::Stop).unwrap();
    }
}

impl MusicIndexBuilder {
    fn add_item(&mut self, p: PathBuf) {
        let extension = match p.extension() {
            Some(e) => e,
            None => return,
        };

        if let Some(format) = AudioFormat::from_extension(extension) {
            let m = Metadata::read_from(&p, format);
            self.send_item(Item::Song((p, format, m.unwrap_or_default())));
        } else if is_image_extension(extension) {
            self.send_item(Item::Image(p));
        } else {
            self.send_item(Item::Other(p));
        }
    }

    fn send_item(&self, item: Item) {
        self.item_sender.send(Msg::Work(item)).unwrap();
    }
}
