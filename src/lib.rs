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
    pub track: u16,
    pub artist: String,
    pub title: String,
    pub current_file: PathBuf,
}

#[derive(Default, Debug, PartialEq)]
pub struct DirCreation {
    pub path: PathBuf,
}

impl DirCreation {
    pub fn excecute(&self) -> Result<(), io::Error> {
        std::fs::create_dir(&self.path)
    }
}

#[derive(Default, Debug, PartialEq)]
pub struct FileOperation {
    pub old: PathBuf,
    pub new: PathBuf,
}

impl FileOperation {
    pub fn excecute(&self, operation: &FileOpType) -> Result<(), io::Error> {
        match operation {
            FileOpType::Copy => fs::copy(&self.old, &self.new).map(|_| ()),
            FileOpType::Move => fs::rename(&self.old, &self.new),
        }
    }
}

#[derive(Debug, PartialEq)]
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
    pub track: u16,
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
                    let track = match tag.track() {
                        Some(t) => t as u16,
                        None => 0,
                    };

                    return Self {
                        track,
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
                    let track = match tag.track_number() {
                        (Some(t), _) => t as u16,
                        (None, _) => 0,
                    };

                    return Self {
                        track,
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
    type Item = Song;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(p) = self.iter.next() {
            let m = Metadata::read_from(&p);
            let song_index = self.index.songs.len();

            let song = Song {
                track: m.track,
                artist: m.artist.clone(),
                title: m.title,
                current_file: p,
            };

            self.index.songs.push(song.clone());

            let artist = if !m.album_artist.is_empty() {
                m.album_artist
            } else if !m.artist.is_empty() {
                m.artist
            } else {
                self.index.unknown.push(song_index);
                return Some(song);
            };

            if self.index.artists.is_empty() {
                self.index.artists.push(Artist {
                    name: artist,
                    albums: vec![Album {
                        name: m.album,
                        songs: vec![song_index],
                    }],
                });

                return Some(song);
            }

            for ar in &mut self.index.artists {
                if ar.name == artist {
                    for al in &mut ar.albums {
                        if al.name == m.album {
                            al.songs.push(song_index);
                            return Some(song);
                        }
                    }

                    ar.albums.push(Album {
                        name: m.album,
                        songs: vec![song_index],
                    });
                    return Some(song);
                }
            }

            self.index.artists.push(Artist {
                name: artist,
                albums: vec![Album {
                    name: m.album,
                    songs: vec![song_index],
                }],
            });

            return Some(song);
        }

        None
    }
}

//fn check() {
//    println!("\nchecking...");
//
//    let mut offset = 1;
//    for ar1 in artists.iter() {
//        for ar2 in artists.iter().skip(offset) {
//            if ar1.name.eq_ignore_ascii_case(&ar2.name) {
//                println!(
//                    "These two artists are named similarly:\n{}\n{}",
//                    &ar1.name, &ar2.name
//                );
//                let index = input_options_loop(&[
//                    "don't do anything",
//                    "merge using first",
//                    "merge using second",
//                    "enter new name",
//                ]);
//
//                match index {
//                    0 => continue,
//                    1 => println!("merging using first"),
//                    2 => println!("merging using second"),
//                    3 => loop {
//                        let new_name = input_loop("enter new name:", |_| true);
//                        println!("new name: '{}'", new_name);
//
//                        let index = input_options_loop(&["ok", "reenter name", "dismiss"]);
//
//                        match index {
//                            0 => {
//                                //TODO: rename
//                                break;
//                            }
//                            1 => continue,
//                            _ => break,
//                        }
//                    },
//                    _ => continue,
//                }
//            }
//        }
//        offset += 1;
//    }
//}

#[derive(Debug, PartialEq)]
pub struct Changes {
    pub output_dir: PathBuf,
    pub dir_creations: Vec<DirCreation>,
    pub file_moves: Vec<FileOperation>,
}

impl Changes {
    pub fn from(index: &MusicIndex, output_dir: PathBuf) -> Self {
        let mut dir_creations = Vec::new();
        let mut file_moves = Vec::with_capacity(index.songs.len() / 10);

        for ar in &index.artists {
            let ar_dir = output_dir.join(valid_os_string(&ar.name));
            if !ar_dir.exists() {
                dir_creations.push(DirCreation {
                    path: ar_dir.clone(),
                });
            }

            for al in &ar.albums {
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

                for si in &al.songs {
                    let song = &index.songs[*si];
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

                        file_name.push(format!("{:02} - ", song.track));
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
            output_dir,
            dir_creations,
            file_moves,
        }
    }

    pub fn write(&self, operation: FileOpType) -> Vec<io::Error> {
        let mut errors = Vec::new();

        if !self.output_dir.exists() {
            if let Err(e) = std::fs::create_dir_all(&self.output_dir) {
                errors.push(e);
            }
        }

        for d in &self.dir_creations {
            if let Err(e) = d.excecute() {
                errors.push(e);
            }
        }

        for f in &self.file_moves {
            if let Err(e) = f.excecute(&operation) {
                errors.push(e);
            }
        }

        errors
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
