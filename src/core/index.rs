use crossbeam_channel::Sender;
use std::path::{Path, PathBuf};

use crate::fs::is_image_extension;
use crate::thread::{worker_pool, Msg, Worker, WorkerState};
use crate::{AudioFormat, Metadata, Song};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MusicIndex<'a> {
    pub music_dir: &'a Path,
    pub songs: Vec<Song>,
    pub unknown: Vec<PathBuf>,
    pub images: Vec<PathBuf>,
}

impl<'a> MusicIndex<'a> {
    pub fn new(music_dir: &'a Path) -> Self {
        Self { music_dir, songs: Vec::new(), unknown: Vec::new(), images: Vec::new() }
    }

    pub fn read(&mut self, f: &mut impl FnMut(&Path)) {
        let (item_sender, item_receiver) = crossbeam_channel::unbounded();

        // 8 threads seems to be the sweat spot on my machine :)
        let num_workers = 8;
        worker_pool(
            num_workers,
            [self.music_dir.to_path_buf()],
            |_| MusicIndexBuilder { item_sender: item_sender.clone() },
            || {
                while let Ok(Msg::Work(i)) = item_receiver.recv() {
                    match i {
                        Item::Song(s) => {
                            f(&s.path);
                            self.songs.push(s);
                        }
                        Item::Unknown(p) => {
                            f(&p);
                            self.unknown.push(p);
                        }
                        Item::Image(p) => {
                            f(&p);
                            self.images.push(p);
                        }
                    }
                }
            },
        );
    }
}

enum Item {
    Song(Song),
    Unknown(PathBuf),
    Image(PathBuf),
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
            self.add_song(p, m.unwrap_or_default(), format);
        } else if is_image_extension(extension) {
            let _ = self.item_sender.send(Msg::Work(Item::Image(p)));
        }
    }

    fn add_song(&mut self, p: PathBuf, m: Metadata, format: AudioFormat) {
        let Some(release_artists) = m.album_artists() else {
            self.send_item(Item::Unknown(p));
            return;
        };

        let Some(song_artists) = m.song_artists() else {
            self.send_item(Item::Unknown(p));
            return;
        };

        let Some(release) = &m.album else {
            self.send_item(Item::Unknown(p));
            return;
        };

        let Some(title) = &m.title else {
            self.send_item(Item::Unknown(p));
            return;
        };

        self.send_item(Item::Song(Song {
            format,
            mode: m.mode,
            track_number: m.track_number,
            total_tracks: m.total_tracks,
            disc_number: m.disc_number,
            total_discs: m.total_discs,
            album_artists: release_artists.to_owned(),
            artists: song_artists.to_owned(),
            album: release.to_owned(),
            title: title.to_owned(),
            genres: m.genres,
            has_artwork: m.has_artwork,
            path: p,
        }));
    }

    fn send_item(&self, item: Item) {
        self.item_sender.send(Msg::Work(item)).unwrap();
    }
}
