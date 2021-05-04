mod fs;
mod meta;
mod update;

pub use fs::{DirCreation, FileOpType, FileOperation};
pub use meta::{Metadata, Release, ReleaseArtists, Song};
pub use update::{TagUpdate, Value};

use fs::is_music_extension;
use std::ffi::OsString;
use std::iter::Iterator;
use std::path::{Path, PathBuf};
use std::{error, io};
use walkdir::WalkDir;

use crate::fs::valid_os_string;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct MusicIndex {
    pub music_dir: PathBuf,
    pub songs: Vec<Song>,
    pub unknown: Vec<PathBuf>,
}

impl MusicIndex {
    pub fn read(&mut self) -> usize {
        ReadMusicIndexIter::from(self).count()
    }

    pub fn read_iter<'a>(&'a mut self) -> ReadMusicIndexIter<'a> {
        ReadMusicIndexIter::from(self)
    }

    pub fn check_missing_artwork(&self, f: &mut impl FnMut(&Song)) {
        for s in &self.songs {
            if !s.has_artwork {
                f(s)
            }
        }
    }
}

impl From<PathBuf> for MusicIndex {
    fn from(music_dir: PathBuf) -> Self {
        Self { music_dir, ..Default::default() }
    }
}

pub struct ReadMusicIndexIter<'a> {
    iter: Box<dyn Iterator<Item = PathBuf>>,
    pub index: &'a mut MusicIndex,
}

impl<'a> From<&'a mut MusicIndex> for ReadMusicIndexIter<'a> {
    fn from(index: &'a mut MusicIndex) -> Self {
        let iter = WalkDir::new(&index.music_dir)
            .follow_links(true)
            .into_iter()
            .filter_entry(|e| !e.file_name().to_str().map(|s| s.starts_with('.')).unwrap_or(false))
            .filter_map(|e| e.ok())
            .filter(|e| e.metadata().map(|m| m.is_file()).unwrap_or(false))
            .filter_map(|e| {
                let p = e.into_path();

                match p.extension().map(is_music_extension) {
                    Some(true) => Some(p),
                    _ => None,
                }
            });

        Self { iter: Box::new(iter), index }
    }
}

impl<'a> Iterator for ReadMusicIndexIter<'a> {
    type Item = PathBuf;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(p) = self.iter.next() {
            let m = Metadata::read_from(&p);

            let release_artists = match m.release_artists() {
                Some(a) => a,
                None => {
                    self.index.unknown.push(p.clone());
                    return Some(p);
                }
            };

            let song_artists = match m.song_artists() {
                Some(a) => a,
                None => {
                    self.index.unknown.push(p.clone());
                    return Some(p);
                }
            };

            let release = match &m.release {
                Some(rl) => rl.clone(),
                None => {
                    self.index.unknown.push(p.clone());
                    return Some(p);
                }
            };

            let title = match &m.title {
                Some(t) => t,
                None => {
                    self.index.unknown.push(p.clone());
                    return Some(p);
                }
            };

            self.index.songs.push(Song {
                track_number: m.track_number,
                total_tracks: m.total_tracks,
                disc_number: m.disc_number,
                total_discs: m.total_discs,
                release_artists: release_artists.to_vec(),
                artists: song_artists.to_vec(),
                release: release.to_owned(),
                title: title.clone(),
                has_artwork: m.has_artwork,
                path: p.clone(),
            });

            return Some(p);
        }

        None
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Changes {
    pub dir_creations: Vec<DirCreation>,
    pub file_operations: Vec<FileOperation>,
}

impl Changes {
    pub fn file_op(&self, path: &Path) -> Option<&FileOperation> {
        self.file_operations.iter().find(|f| &f.old == path)
    }

    //pub fn tag_update(&self, path: &Path) -> Option<&TagUpdate> {
    //    self.file_op(path).and_then(|f| f.tag_update.as_ref())
    //}

    pub fn update_file_op(&mut self, path: &PathBuf, f: impl FnOnce(&mut FileOperation)) {
        match self.file_operations.iter_mut().find(|f| &f.old == path) {
            Some(fo) => f(fo),
            None => {
                let mut fo = FileOperation { old: path.clone(), ..Default::default() };

                f(&mut fo);

                self.file_operations.push(fo);
            }
        }
    }

    //pub fn update_tag(&mut self, path: &PathBuf, f: impl FnOnce(&mut TagUpdate)) {
    //    self.update_file_op(path, |fo| match &mut fo.tag_update {
    //        Some(tu) => f(tu),
    //        None => {
    //            let mut tu = TagUpdate::default();

    //            f(&mut tu);

    //            fo.tag_update = Some(tu);
    //        }
    //    });
    //}

    pub fn check_dir_creation(&mut self, path: &PathBuf) -> bool {
        if !self.dir_creations.iter().any(|d| &d.path == path) && !path.exists() {
            self.dir_creations.push(DirCreation { path: path.clone() });
            true
        } else {
            false
        }
    }

    //pub fn check_inconsitent_release_artists(
    //    &mut self,
    //    index: &MusicIndex,
    //    f: fn(&MusicIndex, &ReleaseArtists, &ReleaseArtists) -> Value<Vec<String>>,
    //) {
    //    let mut offset = 1;
    //    for ar1 in index.artists.iter() {
    //        for ar2 in index.artists.iter().skip(offset) {
    //            for (n1, n2) in ar1.names.iter().zip(ar2.names.iter()) {
    //                if !n1.eq_ignore_ascii_case(n2) {
    //                    continue;
    //                }
    //            }
    //            match f(index, ar1, ar2) {
    //                Value::Update(names) => {
    //                    if ar1.names != names {
    //                        for rl in ar1.releases.iter() {
    //                            for song in rl.songs.iter().map(|&si| &index.songs[si]) {
    //                                self.update_tag(&song.path, |tu| {
    //                                    tu.album_artists = Value::Update(names)
    //                                });
    //                            }
    //                        }
    //                    }

    //                    if ar2.names != names {
    //                        for rl in ar2.releases.iter() {
    //                            for song in rl.songs.iter().map(|&si| &index.songs[si]) {
    //                                self.update_tag(&song.path, |tu| {
    //                                    tu.album_artists = Value::Update(names)
    //                                });
    //                            }
    //                        }
    //                    }
    //                }
    //                Value::Remove => {
    //                    for rl in ar1.releases.iter() {
    //                        for song in rl.songs.iter().map(|&si| &index.songs[si]) {
    //                            self.update_tag(&song.path, |tu| tu.album_artists = Value::Remove);
    //                        }
    //                    }

    //                    for rl in ar2.releases.iter() {
    //                        for song in rl.songs.iter().map(|&si| &index.songs[si]) {
    //                            self.update_tag(&song.path, |tu| tu.album_artists = Value::Remove);
    //                        }
    //                    }
    //                }
    //                Value::Unchanged => (),
    //            }
    //        }
    //        offset += 1;
    //    }
    //}

    //pub fn check_inconsitent_albums(
    //    &mut self,
    //    index: &MusicIndex,
    //    f: fn(&MusicIndex, &ReleaseArtists, &Release, &Release) -> Value<String>,
    //) {
    //    for ar in index.artists.iter() {
    //        let mut offset = 1;
    //        for al1 in ar.releases.iter() {
    //            for al2 in ar.releases.iter().skip(offset) {
    //                if al1.name.eq_ignore_ascii_case(&al2.name) {
    //                    match f(index, ar, al1, al2) {
    //                        Value::Update(name) => {
    //                            if al1.name != name {
    //                                for song in al1.songs.iter().map(|&si| &index.songs[si]) {
    //                                    self.update_tag(&song.path, |tu| {
    //                                        tu.album = Value::Update(name.clone());
    //                                    });
    //                                }
    //                            }

    //                            if al2.name != name {
    //                                for song in al2.songs.iter().map(|&si| &index.songs[si]) {
    //                                    self.update_tag(&song.path, |tu| {
    //                                        tu.album = Value::Update(name.clone());
    //                                    });
    //                                }
    //                            }
    //                        }
    //                        Value::Remove => {
    //                            for song in al1.songs.iter().map(|&si| &index.songs[si]) {
    //                                self.update_tag(&song.path, |tu| {
    //                                    tu.album = Value::Remove;
    //                                });
    //                            }

    //                            for song in al2.songs.iter().map(|&si| &index.songs[si]) {
    //                                self.update_tag(&song.path, |tu| {
    //                                    tu.album = Value::Remove;
    //                                });
    //                            }
    //                        }
    //                        Value::Unchanged => (),
    //                    }
    //                }
    //            }
    //            offset += 1;
    //        }
    //    }
    //}

    //pub fn check_inconsitent_total_tracks(
    //    &mut self,
    //    index: &MusicIndex,
    //    f: fn(&ReleaseArtists, &Release, Vec<(Vec<&Song>, Option<u16>)>) -> Value<u16>,
    //) {
    //    for ar in index.artists.iter() {
    //        for al in ar.releases.iter() {
    //            let mut total_tracks: Vec<(Vec<&Song>, Option<u16>)> = Vec::new();

    //            'songs: for s in al.songs.iter().map(|&si| &index.songs[si]) {
    //                for (songs, tt) in total_tracks.iter_mut() {
    //                    if *tt == s.total_tracks {
    //                        songs.push(s);
    //                        continue 'songs;
    //                    }
    //                }

    //                total_tracks.push((vec![s], s.total_tracks));
    //            }

    //            if total_tracks.len() > 1 {
    //                if let Value::Update(t) = f(ar, al, total_tracks) {
    //                    for song in al.songs.iter().map(|&si| &index.songs[si]) {
    //                        self.update_tag(&song.path, |tu| {
    //                            tu.total_tracks = Value::Update(t);
    //                        });
    //                    }
    //                }
    //            }
    //        }
    //    }
    //}

    //pub fn check_inconsitent_total_discs(
    //    &mut self,
    //    index: &MusicIndex,
    //    f: fn(&ReleaseArtists, &Release, Vec<(Vec<&Song>, Option<u16>)>) -> Value<u16>,
    //) {
    //    for ar in index.artists.iter() {
    //        for rl in ar.releases.iter() {
    //            let mut total_discs: Vec<(Vec<&Song>, Option<u16>)> = Vec::new();

    //            'songs: for s in rl.songs.iter().map(|&si| &index.songs[si]) {
    //                for (songs, tt) in total_discs.iter_mut() {
    //                    if *tt == s.total_discs {
    //                        songs.push(s);
    //                        continue 'songs;
    //                    }
    //                }

    //                total_discs.push((vec![s], s.total_discs));
    //            }

    //            if total_discs.len() > 1 {
    //                match f(ar, rl, total_discs) {
    //                    Value::Update(t) => {
    //                        for song in rl.songs.iter().map(|&si| &index.songs[si]) {
    //                            self.update_tag(&song.path, |tu| tu.total_discs = Value::Update(t));
    //                        }
    //                    }
    //                    Value::Remove => {
    //                        for song in rl.songs.iter().map(|&si| &index.songs[si]) {
    //                            self.update_tag(&song.path, |tu| tu.total_discs = Value::Remove);
    //                        }
    //                    }
    //                    Value::Unchanged => (),
    //                }
    //            }
    //        }
    //    }
    //}

    pub fn file_system(&mut self, index: &MusicIndex, output_dir: &PathBuf) {
        if !output_dir.exists() {
            self.dir_creations.push(DirCreation { path: output_dir.clone() })
        }

        for song in index.songs.iter() {
            let release_artists = valid_os_string(&song.release_artists_str());
            let artists = valid_os_string(&song.artists_str());
            let release = valid_os_string(&song.release);
            let title = valid_os_string(&song.title);
            let extension = song.path.extension().unwrap();
            let track = match song.track_number {
                Some(n) => n,
                _ => 0,
            };

            let mut path = output_dir.join(&release_artists);
            self.check_dir_creation(&path);

            path.push(&release);
            self.check_dir_creation(&path);

            let mut file_name = OsString::new();
            file_name.push(format!("{:02} - ", track));
            file_name.push(artists);
            file_name.push(" - ");
            file_name.push(title);
            file_name.push(".");
            file_name.push(extension);

            path.push(file_name);

            if path != song.path {
                self.update_file_op(&song.path, |fo| fo.new = Some(path))
            }
        }

        if !index.unknown.is_empty() {
            let unknown_dir = output_dir.join("unknown");
            self.check_dir_creation(&unknown_dir);

            for path in index.unknown.iter() {
                let new_file = unknown_dir.join(path.file_name().unwrap());

                self.update_file_op(path, |fo| fo.new = Some(new_file));
            }
        }
    }

    pub fn write(&self, op_type: FileOpType) -> Vec<Box<dyn error::Error>> {
        let mut errors: Vec<Box<dyn error::Error>> = Vec::new();

        for d in &self.dir_creations {
            if let Err(e) = d.execute() {
                errors.push(Box::new(e));
            }
        }

        for f in &self.file_operations {
            if let Err(e) = f.execute(op_type) {
                errors.push(e);
            }
        }

        errors
    }

    pub fn dir_creation_iter(&self) -> DirCreationIter {
        DirCreationIter::from(self)
    }

    pub fn file_operation_iter(&self, op_type: FileOpType) -> FileOperationIter {
        FileOperationIter::from(self, op_type)
    }
}

pub struct DirCreationIter<'a> {
    iter: Box<dyn Iterator<Item = &'a DirCreation> + 'a>,
}

impl<'a> From<&'a Changes> for DirCreationIter<'a> {
    fn from(changes: &'a Changes) -> Self {
        Self { iter: Box::new(changes.dir_creations.iter()) }
    }
}

impl<'a> Iterator for DirCreationIter<'a> {
    type Item = (&'a DirCreation, Result<(), io::Error>);

    fn next(&mut self) -> Option<Self::Item> {
        let d = self.iter.next()?;
        let r = d.execute();

        Some((d, r))
    }
}

pub struct FileOperationIter<'a> {
    iter: Box<dyn Iterator<Item = &'a FileOperation> + 'a>,
    op_type: FileOpType,
}

impl<'a> FileOperationIter<'a> {
    pub fn from(changes: &'a Changes, op_type: FileOpType) -> Self {
        Self { iter: Box::new(changes.file_operations.iter()), op_type }
    }
}

impl<'a> Iterator for FileOperationIter<'a> {
    type Item = (&'a FileOperation, Result<(), Box<dyn error::Error>>);

    fn next(&mut self) -> Option<Self::Item> {
        let f = self.iter.next()?;
        let r = f.execute(self.op_type);

        Some((f, r))
    }
}
