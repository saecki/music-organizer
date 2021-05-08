use std::ffi::OsString;
use std::path::Path;
use std::{error, io};

use crate::Checks;
use crate::Song;
use crate::{
    fs::{valid_os_str, valid_os_str_dots},
    FileOperation,
};
use crate::{DirCreation, FileOpType, MusicIndex, SongOperation};

#[derive(Clone, Debug, PartialEq)]
pub struct Changes<'a> {
    pub index: &'a MusicIndex,
    pub dir_creations: Vec<DirCreation>,
    pub song_operations: Vec<SongOperation<'a>>,
    pub file_operations: Vec<FileOperation<'a>>,
}

impl<'a> Changes<'a> {
    pub fn generate(checks: Checks<'a>, output_dir: &Path) -> Self {
        let mut new = Changes {
            index: checks.index,
            dir_creations: Vec::new(),
            song_operations: checks.updates,
            file_operations: Vec::new(),
        };
        new.generate_diff(output_dir);
        new
    }
}

impl<'a> Changes<'a> {
    fn new_song_path(&self, song: &'a Song) -> &Path {
        if let Some(o) = self.song_operations.iter().find(|o| o.song == song) {
            if let Some(p) = &o.new_path {
                return p;
            }
        }

        &song.path
    }

    fn update_song_op(&mut self, song: &'a Song, f: impl FnOnce(&mut SongOperation)) {
        match self.song_operations.iter_mut().find(|f| f.song == song) {
            Some(fo) => f(fo),
            None => {
                let mut fo = SongOperation { song, tag_update: None, new_path: None };

                f(&mut fo);

                self.song_operations.push(fo);
            }
        }
    }

    fn dir_creation(&mut self, path: &Path) -> bool {
        if !self.dir_creations.iter().any(|d| d.path == path) && !path.exists() {
            self.dir_creations.push(DirCreation { path: path.to_owned() });
            true
        } else {
            false
        }
    }

    fn generate_diff(&mut self, output_dir: &Path) {
        self.dir_creations.clear();
        self.song_operations.clear();

        if !output_dir.exists() {
            self.dir_creations.push(DirCreation { path: output_dir.to_owned() })
        }

        for song in self.index.songs.iter() {
            let release_artists = valid_os_str_dots(&song.release_artists_str());
            let release = valid_os_str_dots(&song.release);

            let artists = valid_os_str(&song.artists_str());
            let title = valid_os_str(&song.title);
            let extension = song.path.extension().unwrap();
            let disc = song.disc_number.unwrap_or(0);
            let total_discs = song.total_discs.unwrap_or(0);
            let track = song.track_number.unwrap_or(0);

            let mut path = output_dir.join(&release_artists);
            self.dir_creation(&path);

            path.push(&release);
            self.dir_creation(&path);

            let mut file_name = OsString::new();
            if total_discs > 1 {
                file_name.push(disc.to_string());
                file_name.push(" ");
            }
            file_name.push(format!("{:02} - ", track));
            file_name.push(&artists);
            file_name.push(" - ");
            file_name.push(&title);
            file_name.push(".");
            file_name.push(extension);

            path.push(file_name);

            if path != song.path {
                self.update_song_op(song, |fo| fo.new_path = Some(path))
            }
        }

        for image in self.index.images.iter() {
            let current_dir = image.parent().unwrap();
            let mut new_song_dirs = self
                .index
                .songs
                .iter()
                .filter(|s| s.path.parent().unwrap() == current_dir)
                .map(|s| self.new_song_path(s).parent().unwrap());

            if let Some(n) = new_song_dirs.next() {
                let new_song_dir = n;

                if new_song_dir == current_dir {
                    continue;
                }

                let mut all_equal = true;
                for n in new_song_dirs {
                    if n != new_song_dir {
                        all_equal = false;
                        break;
                    }
                }

                if all_equal {
                    let new_path = new_song_dir.join(image.file_name().unwrap());
                    self.file_operations.push(FileOperation { old_path: image, new_path });
                }
            }
        }

        if !self.index.unknown.is_empty() {
            let unknown_dir = output_dir.join("unknown");
            self.dir_creation(&unknown_dir);

            for unknown in self.index.unknown.iter() {
                let new_path = unknown_dir.join(unknown.file_name().unwrap());

                if &new_path != unknown {
                    self.file_operations.push(FileOperation { old_path: unknown, new_path });
                }
            }
        }
    }

    pub fn dir_creations(&self, f: &mut impl FnMut(&DirCreation, io::Result<()>)) {
        for d in self.dir_creations.iter() {
            let r = d.execute();
            f(d, r);
        }
    }

    pub fn song_operations(
        &self,
        op_type: FileOpType,
        f: &mut impl FnMut(&SongOperation, Result<(), Box<dyn error::Error>>),
    ) {
        for o in self.song_operations.iter() {
            let r = o.execute(op_type);
            f(o, r);
        }
    }

    pub fn file_operations(
        &self,
        op_type: FileOpType,
        f: &mut impl FnMut(&FileOperation, Result<(), Box<dyn error::Error>>),
    ) {
        for o in self.file_operations.iter() {
            let r = o.execute(op_type);
            f(o, r);
        }
    }
}
