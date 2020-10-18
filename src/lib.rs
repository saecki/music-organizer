use std::ffi::{OsStr, OsString};
use std::iter::Iterator;
use std::path::PathBuf;
use std::{fs, io};
use walkdir::WalkDir;

const MUSIC_FILE_EXTENSIONS: [&str; 5] = ["m4a", "mp3", "m4b", "m4p", "m4v"];

#[derive(Clone, Debug, PartialEq)]
pub struct Artist {
    pub name: String,
    pub albums: Vec<Album>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Album {
    pub name: String,
    pub songs: Vec<usize>,
}

#[derive(Clone, Default, Debug, PartialEq)]
pub struct Song {
    pub track: Option<u16>,
    pub total_tracks: Option<u16>,
    pub disc: Option<u16>,
    pub total_discs: Option<u16>,
    pub artist: String,
    pub title: String,
    pub current_file: PathBuf,
}

#[derive(Default, Debug, PartialEq)]
pub struct DirCreation {
    pub path: PathBuf,
}

impl DirCreation {
    pub fn execute(&self) -> Result<(), io::Error> {
        std::fs::create_dir(&self.path)
    }
}

#[derive(Default, Debug, PartialEq)]
pub struct FileOperation {
    pub old: PathBuf,
    pub new: PathBuf,
}

impl FileOperation {
    pub fn execute(&self, op_type: FileOpType) -> Result<(), io::Error> {
        match op_type {
            FileOpType::Copy => fs::copy(&self.old, &self.new).map(|_| ()),
            FileOpType::Move => fs::rename(&self.old, &self.new),
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum FileOpType {
    Move,
    Copy,
}

impl From<bool> for FileOpType {
    fn from(copy: bool) -> Self {
        match copy {
            true => Self::Copy,
            false => Self::Move,
        }
    }
}

#[derive(Default, Debug, PartialEq)]
pub struct Metadata {
    pub track: Option<u16>,
    pub total_tracks: Option<u16>,
    pub disc: Option<u16>,
    pub total_discs: Option<u16>,
    pub artist: String,
    pub album_artist: String,
    pub album: String,
    pub title: String,
}

impl Metadata {
    pub fn read_from(path: &PathBuf) -> Self {
        match path.extension().unwrap().to_str().unwrap() {
            "mp3" => {
                if let Ok(tag) = id3::Tag::read_from_path(&path) {
                    return Self {
                        track: tag.track().map(|v| v as u16),
                        total_tracks: tag.total_tracks().map(|v| v as u16),
                        disc: tag.disc().map(|v| v as u16),
                        total_discs: tag.total_discs().map(|v| v as u16),
                        artist: tag.artist().unwrap_or("").to_string(),
                        album_artist: tag.album_artist().unwrap_or("").to_string(),
                        title: tag.title().unwrap_or("").to_string(),
                        album: tag.album().unwrap_or("").to_string(),
                    };
                } else {
                }
            }
            "m4a" | "m4b" | "m4p" | "m4v" => {
                if let Ok(tag) = mp4ameta::Tag::read_from_path(&path) {
                    let track = tag.track_number();
                    let disc = tag.disc_number();
                    return Self {
                        track: track.0,
                        total_tracks: track.1,
                        disc: disc.0,
                        total_discs: disc.1,
                        artist: tag.artist().unwrap_or("").to_string(),
                        album_artist: tag.album_artist().unwrap_or("").to_string(),
                        title: tag.title().unwrap_or("").to_string(),
                        album: tag.album().unwrap_or("").to_string(),
                    };
                }
            }
            _ => (),
        }

        Self::default()
    }
}

#[derive(Default, Debug, PartialEq)]
pub struct MusicIndex {
    pub music_dir: PathBuf,
    pub songs: Vec<Song>,
    pub artists: Vec<Artist>,
    pub unknown: Vec<usize>,
}

impl MusicIndex {
    pub fn read(&mut self) -> usize {
        ReadMusicIndexIter::from(self).count()
    }

    pub fn read_iter<'a>(&'a mut self) -> ReadMusicIndexIter<'a> {
        ReadMusicIndexIter::from(self)
    }
}

impl From<PathBuf> for MusicIndex {
    fn from(music_dir: PathBuf) -> Self {
        Self {
            music_dir,
            ..Default::default()
        }
    }
}

pub struct ReadMusicIndexIter<'a> {
    iter: Box<dyn Iterator<Item = PathBuf>>,
    pub index: &'a mut MusicIndex,
}

impl<'a> From<&'a mut MusicIndex> for ReadMusicIndexIter<'a> {
    fn from(index: &'a mut MusicIndex) -> Self {
        let iter = WalkDir::new(&index.music_dir)
            .into_iter()
            .filter_entry(|e| {
                !e.file_name()
                    .to_str()
                    .map(|s| s.starts_with('.'))
                    .unwrap_or(false)
            })
            .filter_map(|e| e.ok())
            .filter(|e| e.metadata().map(|m| m.is_file()).unwrap_or(false))
            .filter_map(|e| {
                let p = e.into_path();

                if is_music_extension(p.extension().unwrap()) {
                    Some(p)
                } else {
                    None
                }
            });

        Self {
            iter: Box::new(iter),
            index,
        }
    }
}

impl<'a> Iterator for ReadMusicIndexIter<'a> {
    type Item = Metadata;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(p) = self.iter.next() {
            let m = Metadata::read_from(&p);
            let song_index = self.index.songs.len();

            let song = Song {
                track: m.track,
                total_tracks: m.total_tracks,
                disc: m.disc,
                total_discs: m.total_discs,
                artist: m.artist.clone(),
                title: m.title.clone(),
                current_file: p,
            };

            self.index.songs.push(song);

            let artist = if !m.album_artist.is_empty() {
                m.album_artist.clone()
            } else if !m.artist.is_empty() {
                m.artist.clone()
            } else {
                self.index.unknown.push(song_index);
                return Some(m);
            };

            if self.index.artists.is_empty() {
                self.index.artists.push(Artist {
                    name: artist,
                    albums: vec![Album {
                        name: m.album.clone(),
                        songs: vec![song_index],
                    }],
                });

                return Some(m);
            }

            for ar in &mut self.index.artists {
                if ar.name == artist {
                    for al in &mut ar.albums {
                        if al.name == m.album {
                            al.songs.push(song_index);
                            return Some(m);
                        }
                    }

                    ar.albums.push(Album {
                        name: m.album.clone(),
                        songs: vec![song_index],
                    });
                    return Some(m);
                }
            }

            self.index.artists.push(Artist {
                name: artist,
                albums: vec![Album {
                    name: m.album.clone(),
                    songs: vec![song_index],
                }],
            });

            return Some(m);
        }

        None
    }
}

pub fn check_inconsitent_artists(
    index: &mut MusicIndex,
    f: impl Fn(&Artist, &Artist) -> Option<String>,
) {
    let mut offset = 1;
    for ar1 in index.artists.iter() {
        for ar2 in index.artists.iter().skip(offset) {
            if ar1.name.eq_ignore_ascii_case(&ar2.name) {
                if let Some(_name) = f(ar1, ar2) {
                    //TODO: update artists
                    unimplemented!()
                }
            }
        }
        offset += 1;
    }
}

pub fn check_inconsitent_albums(
    index: &mut MusicIndex,
    f: impl Fn(&Artist, &Album, &Album) -> Option<String>,
) {
    for ar in index.artists.iter() {
        let mut offset = 1;
        for al1 in ar.albums.iter() {
            for al2 in ar.albums.iter().skip(offset) {
                if al1.name.eq_ignore_ascii_case(&al2.name) {
                    if let Some(_name) = f(ar, al1, al2) {
                        //TODO: update albums
                        unimplemented!()
                    }
                }
            }
            offset += 1;
        }
    }
}

pub fn check_inconsitent_total_tracks(
    index: &MusicIndex,
    f: impl Fn(&Artist, &Album, Vec<Option<u16>>) -> Option<Option<usize>>,
) {
    for ar in index.artists.iter() {
        for al in ar.albums.iter() {
            let mut total_tracks: Vec<Option<u16>> = al
                .songs
                .iter()
                .map(|&si| &index.songs[si])
                .map(|s| s.total_tracks)
                .collect();

            total_tracks.sort();
            total_tracks.dedup();
            if total_tracks.len() > 1 {
                if let Some(_t) = f(ar, al, total_tracks) {
                    //TODO: update tags
                    unimplemented!()
                }
            }
        }
    }
}

pub fn check_inconsitent_total_discs(
    index: &MusicIndex,
    f: impl Fn(&Artist, &Album, Vec<Option<u16>>) -> Option<Option<usize>>,
) {
    for ar in index.artists.iter() {
        for al in ar.albums.iter() {
            let mut total_discs: Vec<Option<u16>> = al
                .songs
                .iter()
                .map(|&si| &index.songs[si])
                .map(|s| s.total_discs)
                .collect();

            total_discs.sort();
            total_discs.dedup();
            if total_discs.len() > 1 {
                if let Some(_t) = f(ar, al, total_discs) {
                    //TODO: update tags
                    unimplemented!()
                }
            }
        }
    }
}

#[derive(Debug, PartialEq)]
pub struct Changes {
    pub dir_creations: Vec<DirCreation>,
    pub file_operations: Vec<FileOperation>,
}

impl Changes {
    pub fn from(index: &MusicIndex, output_dir: &PathBuf) -> Self {
        let mut dir_creations = Vec::new();
        let mut file_moves = Vec::with_capacity(index.songs.len() / 10);

        if !output_dir.exists() {
            dir_creations.push(DirCreation {
                path: output_dir.clone(),
            })
        }

        for ar in index.artists.iter() {
            let ar_dir = output_dir.join(valid_os_string(&ar.name));
            if !ar_dir.exists() {
                dir_creations.push(DirCreation {
                    path: ar_dir.clone(),
                });
            }

            for al in ar.albums.iter() {
                let single_album_name =
                    format!("{} - single", index.songs[al.songs[0]].title.to_lowercase());
                let is_single = al.name.is_empty()
                    || al.songs.len() == 1 && al.name.to_lowercase() == single_album_name;
                let al_dir = ar_dir.join(valid_os_string(&al.name));

                if !is_single && !al_dir.exists() {
                    dir_creations.push(DirCreation {
                        path: al_dir.clone(),
                    });
                }

                for song in al.songs.iter().map(|&si| &index.songs[si]) {
                    let extension = song.current_file.extension().unwrap();

                    let new_file;
                    if is_single {
                        let mut file_name = OsString::with_capacity(
                            4 + song.artist.len() + song.title.len() + extension.len(),
                        );

                        file_name.push(valid_os_string(&song.artist));
                        file_name.push(" - ");
                        file_name.push(valid_os_string(&song.title));
                        file_name.push(".");
                        file_name.push(extension);

                        new_file = ar_dir.join(file_name);
                    } else {
                        let mut file_name = OsString::with_capacity(
                            9 + song.artist.len() + song.title.len() + extension.len(),
                        );

                        file_name.push(format!("{:02} - ", song.track.unwrap_or(0)));
                        file_name.push(valid_os_string(&song.artist));
                        file_name.push(" - ");
                        file_name.push(valid_os_string(&song.title));
                        file_name.push(".");
                        file_name.push(extension);

                        new_file = al_dir.join(file_name);
                    }

                    if new_file != song.current_file {
                        file_moves.push(FileOperation {
                            old: song.current_file.clone(),
                            new: new_file,
                        });
                    }
                }
            }
        }

        if !index.unknown.is_empty() {
            let unknown_dir = output_dir.join("unknown");
            if !unknown_dir.exists() {
                dir_creations.push(DirCreation {
                    path: unknown_dir.clone(),
                });
            }
            for si in &index.unknown {
                let song = &index.songs[*si];
                let new_file = unknown_dir.join(song.current_file.file_name().unwrap());

                file_moves.push(FileOperation {
                    old: song.current_file.clone(),
                    new: new_file,
                });
            }
        }

        Self {
            dir_creations,
            file_operations: file_moves,
        }
    }

    pub fn write(&self, op_type: FileOpType) -> Vec<io::Error> {
        let mut errors = Vec::new();

        for d in &self.dir_creations {
            if let Err(e) = d.execute() {
                errors.push(e);
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
        Self {
            iter: Box::new(changes.dir_creations.iter()),
        }
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
        Self {
            iter: Box::new(changes.file_operations.iter()),
            op_type,
        }
    }
}

impl<'a> Iterator for FileOperationIter<'a> {
    type Item = (&'a FileOperation, Result<(), io::Error>);

    fn next(&mut self) -> Option<Self::Item> {
        let f = self.iter.next()?;
        let r = f.execute(self.op_type);

        Some((f, r))
    }
}

#[inline]
fn is_music_extension(s: &OsStr) -> bool {
    for e in &MUSIC_FILE_EXTENSIONS {
        if s.eq(*e) {
            return true;
        }
    }

    false
}

lazy_static::lazy_static! {
    static ref RE: regex::Regex = regex::Regex::new(r#"[<>:"/\|?*]"#).unwrap();
}

fn valid_os_string(str: &str) -> OsString {
    let mut s = RE.replace_all(str, "").to_string();

    if s.starts_with('.') {
        s.replace_range(0..1, "_");
    }

    if s.ends_with('.') {
        s.replace_range(s.len() - 1..s.len(), "_");
    }

    OsString::from(s)
}
