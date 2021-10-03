use std::path::{Path, PathBuf};

use walkdir::WalkDir;

use crate::fs::{is_image_extension, is_music_extension};
use crate::{Metadata, Song};

#[derive(Clone, Debug, Default, PartialEq)]
pub struct MusicIndex {
    pub music_dir: PathBuf,
    pub songs: Vec<Song>,
    pub unknown: Vec<PathBuf>,
    pub images: Vec<PathBuf>,
}

impl MusicIndex {
    pub fn read(&mut self, f: &mut impl FnMut(&Path)) {
        let iter = WalkDir::new(&self.music_dir)
            .follow_links(true)
            .into_iter()
            .filter_entry(|e| !e.file_name().to_str().map_or(false, |s| s.starts_with('.')))
            .filter_map(|e| e.ok())
            .filter(|e| e.metadata().map_or(false, |m| m.is_file()))
            .map(|e| e.into_path());

        for p in iter {
            f(&p);

            let extension = match p.extension() {
                Some(e) => e,
                None => continue,
            };

            if is_music_extension(extension) {
                let m = Metadata::read_from(&p);

                let release_artists = match m.release_artists() {
                    Some(a) => a,
                    None => {
                        self.unknown.push(p.clone());
                        continue;
                    }
                };

                let song_artists = match m.song_artists() {
                    Some(a) => a,
                    None => {
                        self.unknown.push(p.clone());
                        continue;
                    }
                };

                let release = match &m.release {
                    Some(rl) => rl,
                    None => {
                        self.unknown.push(p.clone());
                        continue;
                    }
                };

                let title = match &m.title {
                    Some(t) => t,
                    None => {
                        self.unknown.push(p.clone());
                        continue;
                    }
                };

                self.songs.push(Song {
                    track_number: m.track_number,
                    total_tracks: m.total_tracks,
                    disc_number: m.disc_number,
                    total_discs: m.total_discs,
                    release_artists: release_artists.to_owned(),
                    artists: song_artists.to_owned(),
                    release: release.to_owned(),
                    title: title.to_owned(),
                    has_artwork: m.has_artwork,
                    path: p.to_owned(),
                });
            } else if is_image_extension(extension) {
                self.images.push(p);
            }
        }
    }
}

impl From<PathBuf> for MusicIndex {
    fn from(music_dir: PathBuf) -> Self {
        Self { music_dir, ..Default::default() }
    }
}
