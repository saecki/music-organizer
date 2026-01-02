use std::collections::BTreeMap;
use std::ffi::{OsStr, OsString};
use std::path::Path;

use indexmap::IndexMap;

use crate::fs::{fast_path_eq, valid_os_str, valid_os_str_dots};
use crate::{
    util, Checks, DirCreation, FileOperation, MoveOrCopy, MusicIndex, Song, SongOperation,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Changes<'a> {
    pub move_or_copy: MoveOrCopy,
    pub index: &'a MusicIndex<'a>,
    pub dir_creations: Vec<DirCreation>,
    pub song_operations: IndexMap<*const Song, SongOperation<'a>>,
    pub file_operations: Vec<FileOperation<'a>>,
}

impl<'a> Changes<'a> {
    pub fn organize(checks: Checks<'a>, output_dir: &Path, move_or_copy: MoveOrCopy) -> Self {
        let mut changes = Changes {
            move_or_copy,
            index: checks.index,
            dir_creations: Vec::new(),
            song_operations: checks.song_operations,
            file_operations: Vec::new(),
        };
        organize_diff(&mut changes, output_dir);
        changes
    }

    fn new_song_path(&self, song: &'a Song) -> &Path {
        self.song_operations
            .get(&(song as *const _))
            .and_then(|op| op.new_path.as_ref())
            .unwrap_or(&song.path)
    }

    fn dir_creation(&mut self, path: &Path) -> bool {
        if !self.dir_creations.iter().any(|d| fast_path_eq(&d.path, path)) && !path.exists() {
            self.dir_creations.push(DirCreation { path: path.to_owned() });
            true
        } else {
            false
        }
    }

    pub fn execute_dir_creations(&self, mut f: impl FnMut(&DirCreation, std::io::Result<()>)) {
        for dc in self.dir_creations.iter() {
            let res = dc.execute();
            f(dc, res);
        }
    }

    pub fn execute_song_operations(&self, mut f: impl FnMut(&SongOperation, anyhow::Result<()>)) {
        for op in self.song_operations.values() {
            let res = op.execute(self.move_or_copy);
            f(op, res);
        }
    }

    pub fn execute_file_operations(&self, mut f: impl FnMut(&FileOperation, anyhow::Result<()>)) {
        for op in self.file_operations.iter() {
            let res = op.execute(self.move_or_copy);
            f(op, res);
        }
    }

    pub fn is_empty(&self) -> bool {
        self.dir_creations.is_empty()
            && self.song_operations.is_empty()
            && self.file_operations.is_empty()
    }
}

fn organize_diff(changes: &mut Changes, output_dir: &Path) {
    if !output_dir.exists() {
        changes.dir_creations.push(DirCreation { path: output_dir.to_owned() })
    }

    let mut release_dirs = BTreeMap::<&OsStr, Vec<&Song>>::new();
    for song in changes.index.songs.iter() {
        let parent_dir = song.path.parent().unwrap();
        match release_dirs.entry(parent_dir.as_os_str()) {
            std::collections::btree_map::Entry::Occupied(occupied) => {
                occupied.into_mut().push(song);
            }
            std::collections::btree_map::Entry::Vacant(vacant) => {
                vacant.insert(vec![song]);
            }
        }

        let op = changes.song_operations.get_mut(&(song as *const Song));
        let tag_update = op.and_then(|op| op.tag_update.as_ref());

        let release_artists = tag_update
            .and_then(|t| t.release_artists.slice_value())
            .unwrap_or(song.album_artists.as_slice())
            .join(", ");
        let release_artists = valid_os_str_dots(&release_artists);

        let release = tag_update.and_then(|t| t.release.str_value()).unwrap_or(&song.album);
        let release = valid_os_str_dots(release);

        let artists = tag_update
            .and_then(|t| t.artists.slice_value())
            .unwrap_or(song.artists.as_slice())
            .join(", ");
        let artists = valid_os_str(&artists);

        let title = tag_update.and_then(|t| t.title.str_value()).unwrap_or(&song.title);
        let title = valid_os_str(title);

        let extension = song.path.extension().unwrap();

        let disc =
            tag_update.and_then(|t| t.disc_number.num_value()).or(song.disc_number).unwrap_or(0);
        let total_discs =
            tag_update.and_then(|t| t.total_discs.num_value()).or(song.total_discs).unwrap_or(0);
        let track =
            tag_update.and_then(|t| t.track_number.num_value()).or(song.track_number).unwrap_or(0);

        let mut path = output_dir.join(release_artists);
        changes.dir_creation(&path);

        path.push(&release);
        changes.dir_creation(&path);

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
            util::update_song_op(&mut changes.song_operations, song, |op| op.new_path = Some(path));
        }
    }

    for image in changes.index.images.iter() {
        let current_dir = image.parent().unwrap();
        let Some(release_dir) = release_dirs.get(current_dir.as_os_str()) else {
            continue;
        };

        let mut new_song_dirs =
            release_dir.iter().map(|s| changes.new_song_path(s).parent().unwrap());

        if let Some(new_song_dir) = new_song_dirs.next() {
            if fast_path_eq(new_song_dir, current_dir) {
                continue;
            }

            let mut all_equal = true;
            for n in new_song_dirs {
                if !fast_path_eq(n, new_song_dir) {
                    all_equal = false;
                    break;
                }
            }

            if all_equal {
                let new_path = new_song_dir.join(image.file_name().unwrap());
                changes.file_operations.push(FileOperation { old_path: image, new_path });
            }
        }
    }

    if !changes.index.unknown_songs.is_empty() {
        let unknown_dir = output_dir.join("unknown");
        changes.dir_creation(&unknown_dir);

        for unknown in changes.index.unknown_songs.iter() {
            let new_path = unknown_dir.join(unknown.path.file_name().unwrap());

            if new_path != unknown.path {
                changes.file_operations.push(FileOperation { old_path: &unknown.path, new_path });
            }
        }
    }
}
