use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::fmt::Write as _;
use std::path::Path;

use indexmap::IndexMap;

use crate::fs::{FileOp, fast_path_eq, valid_os_str, valid_os_str_dots};
use crate::meta::FilePath;
use crate::{
    AudioFormat, Checks, CopyFileOp, CreateDirOp, DeleteFileOp, DirState, Metadata, MusicIndex,
    Song, SongOp, TranscodeFormat, TranscodeOp, util,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OrganizeChanges<'a> {
    pub index: &'a MusicIndex<'a>,
    dir_creations: IndexMap<CreateDirOp, DirState>,
    pub song_ops: IndexMap<*const Song, SongOp<'a>>,
    pub file_ops: IndexMap<*const Path, FileOp<'a>>,
}

impl<'a> OrganizeChanges<'a> {
    pub fn generate(checks: Checks<'a>) -> Self {
        let mut changes = Self {
            index: checks.index,
            dir_creations: IndexMap::new(),
            song_ops: checks.song_ops,
            file_ops: checks.file_ops,
        };
        organize_diff(&mut changes);
        changes
    }
    pub fn dir_creations(&self) -> impl Iterator<Item = &CreateDirOp> {
        self.dir_creations
            .iter()
            .filter_map(|(dc, state)| (*state == DirState::Missing).then_some(dc))
    }

    pub fn is_empty(&self) -> bool {
        self.dir_creations.is_empty() && self.song_ops.is_empty() && self.file_ops.is_empty()
    }

    fn new_song_path(&self, song: &'a Song) -> &Path {
        self.song_ops
            .get(&(song as *const _))
            .and_then(|op| op.new_path.as_ref())
            .unwrap_or(&song.path)
    }
}

fn organize_diff(changes: &mut OrganizeChanges) {
    let mut release_dirs = BTreeMap::<&OsStr, Vec<&Song>>::new();
    for song in changes.index.songs.iter() {
        let parent_dir = song.path.parent().unwrap();
        release_dirs.entry(parent_dir.as_os_str()).or_default().push(song);

        let op = changes.song_ops.get_mut(&(song as *const Song));
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

        let extension = song.format.extension();

        let disc =
            tag_update.and_then(|t| t.disc_number.num_value()).or(song.disc_number).unwrap_or(0);
        let total_discs =
            tag_update.and_then(|t| t.total_discs.num_value()).or(song.total_discs).unwrap_or(0);
        let track =
            tag_update.and_then(|t| t.track_number.num_value()).or(song.track_number).unwrap_or(0);

        let mut path = changes.index.root.join(release_artists);

        if !song.path.starts_with(&path) {
            util::create_dir_op(&mut changes.dir_creations, &path);
        }

        path.push(&release);
        if !song.path.starts_with(&path) {
            util::create_dir_op(&mut changes.dir_creations, &path);
        }

        let mut file_name = String::new();
        if total_discs > 1 {
            _ = write!(&mut file_name, "{disc} ");
        }
        _ = write!(&mut file_name, "{track:02} - {artists} - {title}.{extension}");

        path.push(file_name);

        if path != song.path {
            util::update_song_op(&mut changes.song_ops, song, |op| op.new_path = Some(path));
        }
    }

    for image in changes.index.images.iter() {
        let current_dir = image.path.parent().unwrap();
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
                let new_path = new_song_dir.join(image.path.file_name().unwrap());
                util::update_file_op(&mut changes.file_ops, &image.path, |op| {
                    op.new_path = Some(new_path);
                });
            }
        }
    }

    if !changes.index.unknown_songs.is_empty() {
        let unknown_dir = changes.index.root.join("unknown");
        util::create_dir_op(&mut changes.dir_creations, &unknown_dir);

        for unknown in changes.index.unknown_songs.iter() {
            let new_path = unknown_dir.join(unknown.path.file_name().unwrap());

            if new_path != unknown.path {
                util::update_file_op(&mut changes.file_ops, &unknown.path, |op| {
                    op.new_path = Some(new_path);
                });
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TranscodeChanges<'a> {
    /// The index of the directory that should be transcoded.
    pub a: &'a MusicIndex<'a>,
    /// The index of the directory in which the transcoded songs should be placed.
    pub b: &'a MusicIndex<'a>,
    dir_creations: IndexMap<CreateDirOp, DirState>,
    pub transcode_ops: Vec<TranscodeOp<'a>>,
    pub copy_ops: Vec<CopyFileOp<'a>>,
    pub delete_ops: Vec<DeleteFileOp<'a>>,
}

impl<'a> TranscodeChanges<'a> {
    pub fn generate(a: &'a MusicIndex<'a>, b: &'a MusicIndex<'a>) -> Self {
        let mut changes = Self {
            a,
            b,
            dir_creations: IndexMap::new(),
            transcode_ops: Vec::new(),
            copy_ops: Vec::new(),
            delete_ops: Vec::new(),
        };
        transcode_diff(&mut changes);
        changes
    }

    pub fn dir_creations(&self) -> impl Iterator<Item = &CreateDirOp> {
        self.dir_creations
            .iter()
            .filter_map(|(dc, state)| (*state == DirState::Missing).then_some(dc))
    }

    pub fn is_empty(&self) -> bool {
        self.dir_creations.is_empty()
            && self.transcode_ops.is_empty()
            && self.copy_ops.is_empty()
            && self.delete_ops.is_empty()
    }
}

fn transcode_diff(changes: &mut TranscodeChanges) {
    // Build lookup table for relative paths in the target directory.
    let mut target_files = IndexMap::new();
    for file in changes.b.all_files_iter() {
        let sub_path = file.path.strip_prefix(changes.b.root).unwrap();
        target_files.insert(sub_path, MarkedFile { file, marked: false });
    }

    // TODO: Generate dir Creations.

    // Transcode or copy songs.
    for song in changes.a.songs.iter() {
        let format = match song.format {
            AudioFormat::Flac => Some(TranscodeFormat::Opus),
            AudioFormat::M4a => None,
            AudioFormat::Mp3 => None,
            AudioFormat::Opus => None,
        };
        let sub_path = song.path.strip_prefix(changes.a.root).unwrap();
        if let Some(format) = format {
            let mut sub_path = sub_path.to_path_buf();
            sub_path.set_extension(format.extension());

            if should_copy_or_transcode_song(&mut target_files, &sub_path, song.meta) {
                let new_path = changes.b.root.join(sub_path);
                create_parents(changes, &new_path);
                changes.transcode_ops.push(TranscodeOp { song, new_path, format });
            }
        } else {
            // Don't transcode, only copy song.
            if should_copy_or_transcode_song(&mut target_files, sub_path, song.meta) {
                let new_path = changes.b.root.join(sub_path);
                create_parents(changes, &new_path);
                changes.copy_ops.push(CopyFileOp { old_path: &song.path, new_path });
            }
        }
    }

    // Copy files.
    for file in changes.a.non_song_files_iter() {
        let sub_path = file.path.strip_prefix(changes.a.root).unwrap();
        if should_copy_or_transcode_song(&mut target_files, sub_path, file.meta) {
            let new_path = changes.b.root.join(sub_path);
            create_parents(changes, &new_path);
            changes.copy_ops.push(CopyFileOp { old_path: file.path, new_path });
        }
    }

    // Delete files.
    for target in target_files.values() {
        if !target.marked {
            changes.delete_ops.push(DeleteFileOp { path: target.file.path });
        }
    }
}

fn create_parents(changes: &mut TranscodeChanges, new_path: &Path) {
    let mut parent = new_path.parent();
    let start_idx = changes.dir_creations.len();
    while let Some(dir) = parent
        && !fast_path_eq(dir, changes.b.root)
    {
        if !util::create_dir_op(&mut changes.dir_creations, dir) {
            break;
        }
        parent = dir.parent();
    }

    // Ideally we could just call reverse on the slice, but `indexmap`s slice
    // type doesn't support that.
    let n = changes.dir_creations[start_idx..].len();
    let head = (0..n / 2).map(|i| start_idx + i);
    let tail = (n.div_ceil(2)..n).rev().map(|i| start_idx + i);
    for (a, b) in head.zip(tail) {
        changes.dir_creations.swap_indices(a, b);
    }
}

struct MarkedFile<'a> {
    file: FilePath<&'a Path>,
    marked: bool,
}

fn should_copy_or_transcode_song(
    target_files: &mut IndexMap<&Path, MarkedFile<'_>>,
    sub_path: &Path,
    meta: Metadata,
) -> bool {
    if let Some(target) = target_files.get_mut(sub_path) {
        target.marked = true;

        // Also replace file, if the file in the target directory is older.
        meta.timestamp > target.file.meta.timestamp
    } else {
        true
    }
}
