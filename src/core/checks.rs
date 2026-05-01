use std::ffi::OsStr;
use std::path::Path;

use indexmap::IndexMap;

use crate::fs::FileOp;
use crate::{MusicIndex, Song, SongOp, Value, util};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Artists<'a> {
    pub names: &'a [String],
    pub releases: Vec<Album<'a>>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Album<'a> {
    pub name: &'a str,
    pub songs: Vec<&'a Song>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Checks<'a> {
    pub index: &'a MusicIndex<'a>,
    pub song_ops: IndexMap<*const Song, SongOp<'a>>,
    pub file_ops: IndexMap<*const Path, FileOp<'a>>,
    pub artists: Vec<Artists<'a>>,
}

impl<'a> Checks<'a> {
    pub fn new(index: &'a MusicIndex<'a>) -> Self {
        let mut new = Self {
            index,
            song_ops: IndexMap::new(),
            file_ops: IndexMap::new(),
            artists: Vec::new(),
        };
        new.update_index();
        new
    }
}

impl<'a> Checks<'a> {
    pub fn update_index(&mut self) {
        self.artists.clear();

        for s in self.index.songs.iter() {
            let mut added = false;

            for a in self.artists.iter_mut() {
                if a.names == s.album_artists {
                    for r in a.releases.iter_mut() {
                        if r.name == s.album {
                            r.songs.push(s);
                            added = true;
                        }
                    }

                    if !added {
                        a.releases.push(Album { name: &s.album, songs: vec![s] });
                        added = true;
                    }
                }
            }

            if !added {
                self.artists.push(Artists {
                    names: &s.album_artists,
                    releases: vec![Album { name: &s.album, songs: vec![s] }],
                });
            }
        }
    }

    pub fn remove_embedded_artworks(&mut self, only_redundant: bool) {
        for song in self.index.songs.iter() {
            if !song.has_artwork {
                continue;
            };

            if only_redundant {
                // FIXME: kinda hacky
                let song_dir = song.path.parent().unwrap();
                let external_cover =
                    self.index.images.iter().any(|image| image.path.starts_with(song_dir));
                if !external_cover {
                    continue;
                }
            }

            util::update_tag(&mut self.song_ops, song, |t| t.artwork = Value::Remove);
        }
    }

    pub fn check_file_permissions(&mut self) {
        for song in self.index.songs.iter() {
            if song.meta.mode.permissions() != 0o755 {
                util::update_song_op(&mut self.song_ops, song, |op| {
                    op.mode_update = Some(song.meta.mode.with_permissions(0o755));
                });
            }
        }
        for file in self.index.non_song_files_iter() {
            // Skip hidden files
            if let Some(name) = file.path.file_name().and_then(OsStr::to_str)
                && name.starts_with(".")
            {
                continue;
            }

            if file.meta.mode.permissions() != 0o755 {
                util::update_file_op(&mut self.file_ops, file.path, |op| {
                    op.mode_update = Some(file.meta.mode.with_permissions(0o755));
                });
            }
        }
        // TODO: Also check non-song files here.
    }

    pub fn check_inconsitent_release_artists(
        &mut self,
        f: fn(&Artists, &Artists) -> Value<Vec<String>>,
    ) {
        let mut offset = 1;
        for ar1 in self.artists.iter() {
            'ar2: for ar2 in self.artists.iter().skip(offset) {
                if ar1.names.len() != ar2.names.len() {
                    continue;
                }
                for (n1, n2) in ar1.names.iter().zip(ar2.names.iter()) {
                    if !n1.eq_ignore_ascii_case(n2) {
                        continue 'ar2;
                    }
                }
                match f(ar1, ar2) {
                    Value::Update(names) => {
                        if ar1.names != names {
                            for rl in ar1.releases.iter() {
                                for song in rl.songs.iter() {
                                    util::update_tag(&mut self.song_ops, song, |tu| {
                                        tu.release_artists = Value::Update(names.clone())
                                    });
                                }
                            }
                        }

                        if ar2.names != names {
                            for rl in ar2.releases.iter() {
                                for song in rl.songs.iter() {
                                    util::update_tag(&mut self.song_ops, song, |tu| {
                                        tu.release_artists = Value::Update(names.clone())
                                    });
                                }
                            }
                        }
                    }
                    Value::Remove => {
                        for rl in ar1.releases.iter() {
                            for song in rl.songs.iter() {
                                util::update_tag(&mut self.song_ops, song, |tu| {
                                    tu.release_artists = Value::Remove
                                });
                            }
                        }

                        for rl in ar2.releases.iter() {
                            for song in rl.songs.iter() {
                                util::update_tag(&mut self.song_ops, song, |tu| {
                                    tu.release_artists = Value::Remove
                                });
                            }
                        }
                    }
                    Value::Unchanged => (),
                }
            }
            offset += 1;
        }
    }
}
