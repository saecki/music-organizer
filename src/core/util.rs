use indexmap::IndexMap;

use crate::{Song, SongOperation, TagUpdate};

pub fn update_song_op<'a>(
    song_operations: &mut IndexMap<*const Song, SongOperation<'a>>,
    song: &'a Song,
    f: impl FnOnce(&mut SongOperation),
) {
    match song_operations.get_mut(&(song as *const Song)) {
        Some(o) => f(o),
        None => {
            let mut o = SongOperation::new(song);
            f(&mut o);
            song_operations.insert(song, o);
        }
    }
}

pub fn update_tag<'a>(
    song_operations: &mut IndexMap<*const Song, SongOperation<'a>>,
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
