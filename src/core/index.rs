use std::path::{Path, PathBuf};
use std::time::Duration;
use std::{fs, thread};

use crossbeam_channel::{Receiver, Sender};

use crate::fs::{is_image_extension, is_song_extension};
use crate::{Metadata, Song};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MusicIndex {
    pub music_dir: PathBuf,
    pub songs: Vec<Song>,
    pub unknown: Vec<PathBuf>,
    pub images: Vec<PathBuf>,
}

struct MusicIndexBuilder {
    dir_receiver: Receiver<PathBuf>,
    dir_sender: Sender<PathBuf>,
    item_sender: Sender<Item>,
}

enum Item {
    Song(Song),
    Unknown(PathBuf),
    Image(PathBuf),
}

impl MusicIndexBuilder {
    fn start(&mut self) {
        while let Ok(p) = self.dir_receiver.recv_timeout(Duration::from_millis(100)) {
            self.read(p);
        }
    }

    fn read(&mut self, dir: PathBuf) {
        if let Ok(r) = fs::read_dir(dir) {
            for e in r.into_iter().filter_map(|e| e.ok()) {
                let p = e.path();

                if p.is_file() {
                    self.add_item(p);
                } else if p.is_dir() {
                    if let Err(e) = self.dir_sender.send(p) {
                        println!("Error indexing subdir: {:?}", e);
                    }
                }
            }
        }
    }

    fn add_item(&mut self, p: PathBuf) {
        let extension = match p.extension() {
            Some(e) => e,
            None => return,
        };

        if is_song_extension(extension) {
            let m = Metadata::read_from(&p);
            self.add_song(p, m);
        } else if is_image_extension(extension) {
            let _ = self.item_sender.send(Item::Image(p));
        }
    }

    fn add_song(&mut self, p: PathBuf, m: Metadata) {
        let release_artists = match m.release_artists() {
            Some(a) => a,
            None => {
                let _ = self.item_sender.send(Item::Unknown(p));
                return;
            }
        };

        let song_artists = match m.song_artists() {
            Some(a) => a,
            None => {
                let _ = self.item_sender.send(Item::Unknown(p));
                return;
            }
        };

        let release = match &m.release {
            Some(rl) => rl,
            None => {
                let _ = self.item_sender.send(Item::Unknown(p));
                return;
            }
        };

        let title = match &m.title {
            Some(t) => t,
            None => {
                let _ = self.item_sender.send(Item::Unknown(p));
                return;
            }
        };

        let _ = self.item_sender.send(Item::Song(Song {
            track_number: m.track_number,
            total_tracks: m.total_tracks,
            disc_number: m.disc_number,
            total_discs: m.total_discs,
            release_artists: release_artists.to_owned(),
            artists: song_artists.to_owned(),
            release: release.to_owned(),
            title: title.to_owned(),
            has_artwork: m.has_artwork,
            path: p,
        }));
    }
}

impl MusicIndex {
    pub fn read(&mut self, f: &mut impl FnMut(&Path)) {
        let (item_sender, item_receiver) = crossbeam_channel::unbounded();
        let (dir_sender, dir_receiver) = crossbeam_channel::unbounded();

        let mut threads = Vec::new();
        for _ in 0..8 {
            let mut builder = MusicIndexBuilder {
                dir_receiver: dir_receiver.clone(),
                dir_sender: dir_sender.clone(),
                item_sender: item_sender.clone(),
            };
            let t = thread::spawn(move || {
                builder.start();
            });
            threads.push(t);
        }

        if let Err(e) = dir_sender.send(self.music_dir.clone()) {
            println!("Error indexing music dir: {:?}", e);
        }

        drop(item_sender);

        while let Ok(i) = item_receiver.recv() {
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

        for t in threads {
            if let Err(e) = t.join() {
                println!("Error joining index builder thread: {:?}", e);
            }
        }
    }
}

impl From<PathBuf> for MusicIndex {
    fn from(music_dir: PathBuf) -> Self {
        Self { music_dir, ..Default::default() }
    }
}
