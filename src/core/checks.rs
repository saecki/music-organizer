use crate::{util, MusicIndex, Release, ReleaseArtists, SongOperation, Value};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Checks<'a> {
    pub index: &'a MusicIndex,
    pub song_operations: Vec<SongOperation<'a>>,
    pub artists: Vec<ReleaseArtists<'a>>,
}

impl<'a> From<&'a MusicIndex> for Checks<'a> {
    fn from(index: &'a MusicIndex) -> Self {
        let mut new = Self { index, song_operations: Vec::new(), artists: Vec::new() };
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
                if a.names == s.release_artists {
                    for r in a.releases.iter_mut() {
                        if r.name == s.release {
                            r.songs.push(s);
                            added = true;
                        }
                    }

                    if !added {
                        a.releases.push(Release { name: &s.release, songs: vec![s] });
                        added = true;
                    }
                }
            }

            if !added {
                self.artists.push(ReleaseArtists {
                    names: &s.release_artists,
                    releases: vec![Release { name: &s.release, songs: vec![s] }],
                });
            }
        }
    }

    pub fn remove_embedded_artworks(&mut self) {
        for song in self.index.songs.iter() {
            if song.has_artwork {
                util::update_tag(&mut self.song_operations, song, |t| t.artwork = Value::Remove);
            }
        }
    }

    pub fn check_file_permissions(&mut self) {
        for song in self.index.songs.iter() {
            if let Some(mode) = song.mode {
                if mode.permissions() != 0o755 {
                    util::update_song_op(&mut self.song_operations, song, |op| {
                        op.mode_update = Some(mode.with_permissions(0o755));
                    });
                }
            }
        }
    }

    pub fn check_inconsitent_release_artists(
        &mut self,
        f: fn(&ReleaseArtists, &ReleaseArtists) -> Value<Vec<String>>,
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
                                    util::update_tag(&mut self.song_operations, song, |tu| {
                                        tu.release_artists = Value::Update(names.clone())
                                    });
                                }
                            }
                        }

                        if ar2.names != names {
                            for rl in ar2.releases.iter() {
                                for song in rl.songs.iter() {
                                    util::update_tag(&mut self.song_operations, song, |tu| {
                                        tu.release_artists = Value::Update(names.clone())
                                    });
                                }
                            }
                        }
                    }
                    Value::Remove => {
                        for rl in ar1.releases.iter() {
                            for song in rl.songs.iter() {
                                util::update_tag(&mut self.song_operations, song, |tu| {
                                    tu.release_artists = Value::Remove
                                });
                            }
                        }

                        for rl in ar2.releases.iter() {
                            for song in rl.songs.iter() {
                                util::update_tag(&mut self.song_operations, song, |tu| {
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
}
