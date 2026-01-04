use std::ffi::OsStr;
use std::path::Path;

use indexmap::IndexMap;

use crate::{CreateDirOp, Song, SongOp, TagUpdate};

/// Skip the more expensive path comparison.
impl std::borrow::Borrow<OsStr> for CreateDirOp {
    fn borrow(&self) -> &OsStr {
        self.path.as_os_str()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DirState {
    Missing,
    Exists,
}

/// Create a directory
pub fn create_dir_op(create_dir_ops: &mut IndexMap<CreateDirOp, DirState>, path: &Path) -> bool {
    if create_dir_ops.get(path.as_os_str()).is_none() {
        let state = if path.exists() { DirState::Exists } else { DirState::Missing };
        create_dir_ops.insert(CreateDirOp { path: path.to_path_buf() }, state);
        true
    } else {
        false
    }
}

pub fn update_song_op<'a>(
    song_operations: &mut IndexMap<*const Song, SongOp<'a>>,
    song: &'a Song,
    f: impl FnOnce(&mut SongOp),
) {
    match song_operations.get_mut(&(song as *const Song)) {
        Some(o) => f(o),
        None => {
            let mut o = SongOp::new(song);
            f(&mut o);
            song_operations.insert(song, o);
        }
    }
}

pub fn update_tag<'a>(
    song_operations: &mut IndexMap<*const Song, SongOp<'a>>,
    song: &'a Song,
    f: impl FnOnce(&mut TagUpdate),
) {
    update_song_op(song_operations, song, |op| match &mut op.tag_update {
        Some(t) => f(t),
        None => {
            let mut t = TagUpdate::default();

            f(&mut t);

            op.tag_update = Some(t);
        }
    });
}
