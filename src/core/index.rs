use crossbeam_channel::Sender;
use std::fs::File;
use std::path::{Path, PathBuf};

use crate::fs::is_image_extension;
use crate::meta::{FilePath, IncompleteSong, Tags};
use crate::thread::{Msg, Worker, WorkerState, worker_pool};
use crate::{AudioFormat, Metadata, Song};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MusicIndex<'a> {
    /// The root directory from which the index has been built.
    pub root: &'a Path,
    /// Songs that have all relevant tags.
    pub songs: Vec<Song>,
    /// Songs that are missing some necessary tags.
    pub unknown_songs: Vec<IncompleteSong>,
    pub images: Vec<FilePath<PathBuf>>,
    pub other: Vec<FilePath<PathBuf>>,
}

impl<'a> MusicIndex<'a> {
    pub fn new(music_dir: &'a Path) -> Self {
        Self {
            root: music_dir,
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
            [self.root.to_path_buf()],
            |_| MusicIndexBuilder { item_sender: item_sender.clone() },
            || {
                while let Ok(Msg::Work(item)) = item_receiver.recv() {
                    f(&item);
                    match item {
                        Item::Song(path, meta, format, tags) => {
                            match Song::try_from(path, format, meta, tags) {
                                Ok(song) => self.songs.push(song),
                                Err(unknown_song) => self.unknown_songs.push(unknown_song),
                            }
                        }
                        Item::Image(path, meta) => self.images.push(FilePath { path, meta }),
                        Item::Other(path, meta) => self.other.push(FilePath { path, meta }),
                        Item::Error(..) => (),
                    }
                }
            },
        );

        self.songs.sort_by(|a, b| a.path.cmp(&b.path));
        self.unknown_songs.sort_by(|a, b| a.path.cmp(&b.path));
        self.images.sort();
        self.other.sort();
    }

    pub fn all_files_iter(&self) -> impl Iterator<Item = FilePath<&Path>> {
        (self.songs.iter().map(|s| s.as_file_ref())).chain(self.non_song_files_iter())
    }

    pub fn non_song_files_iter(&self) -> impl Iterator<Item = FilePath<&Path>> {
        (self.unknown_songs.iter().map(|s| s.as_file_ref()))
            .chain(self.images.iter().map(|s| s.as_file_ref()))
            .chain(self.other.iter().map(|s| s.as_file_ref()))
    }
}

pub enum Item {
    Song(PathBuf, Metadata, AudioFormat, Tags),
    Image(PathBuf, Metadata),
    Other(PathBuf, Metadata),
    Error(PathBuf, anyhow::Error),
}

impl Item {
    pub fn path(&self) -> &Path {
        match self {
            Self::Song(path, ..) => path,
            Self::Image(path, ..) => path,
            Self::Other(path, ..) => path,
            Self::Error(path, ..) => path,
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
    fn add_item(&mut self, path: PathBuf) {
        let (mut file, meta) = match open_file(&path) {
            Ok(f) => f,
            Err(err) => {
                self.send_item(Item::Error(path, err));
                return;
            }
        };

        let Some(extension) = path.extension() else {
            self.send_item(Item::Other(path, meta));
            return;
        };

        if let Some(format) = AudioFormat::from_extension(extension) {
            let item = match Tags::read_from(&mut file, format) {
                Ok(tags) => Item::Song(path, meta, format, tags),
                Err(err) => Item::Error(path, err),
            };
            self.send_item(item);
        } else if is_image_extension(extension) {
            self.send_item(Item::Image(path, meta));
        } else {
            self.send_item(Item::Other(path, meta));
        }
    }

    fn send_item(&self, item: Item) {
        self.item_sender.send(Msg::Work(item)).unwrap();
    }
}

fn open_file(path: &Path) -> anyhow::Result<(File, Metadata)> {
    let mut file = File::open(path)?;
    let meta = Metadata::read(&mut file)?;
    Ok((file, meta))
}
