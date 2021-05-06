use std::ffi::OsString;
use std::path::Path;
use std::{error, io};

use crate::fs::{valid_os_str, valid_os_str_dots};
use crate::{DirCreation, FileOpType, FileOperation, MusicIndex};

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Changes {
    pub dir_creations: Vec<DirCreation>,
    pub file_operations: Vec<FileOperation>,
}

impl Changes {
    pub fn file_op(&self, path: &Path) -> Option<&FileOperation> {
        self.file_operations.iter().find(|f| f.old == path)
    }

    //pub fn tag_update(&self, path: &Path) -> Option<&TagUpdate> {
    //    self.file_op(path).and_then(|f| f.tag_update.as_ref())
    //}

    pub fn update_file_op(&mut self, path: &Path, f: impl FnOnce(&mut FileOperation)) {
        match self.file_operations.iter_mut().find(|f| f.old == path) {
            Some(fo) => f(fo),
            None => {
                let mut fo = FileOperation { old: path.to_owned(), ..Default::default() };

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

    pub fn check_dir_creation(&mut self, path: &Path) -> bool {
        if !self.dir_creations.iter().any(|d| d.path == path) && !path.exists() {
            self.dir_creations.push(DirCreation { path: path.to_owned() });
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

    pub fn file_system(&mut self, index: &MusicIndex, output_dir: &Path) {
        if !output_dir.exists() {
            self.dir_creations.push(DirCreation { path: output_dir.to_owned() })
        }

        for song in index.songs.iter() {
            let release_artists = valid_os_str_dots(&song.release_artists_str());
            let release = valid_os_str_dots(&song.release);

            let artists = valid_os_str(&song.artists_str());
            let title = valid_os_str(&song.title);
            let extension = song.path.extension().unwrap();
            let track = song.track_number.unwrap_or(0);

            let mut path = output_dir.join(&release_artists);
            self.check_dir_creation(&path);

            path.push(&release);
            self.check_dir_creation(&path);

            let mut file_name = OsString::new();
            file_name.push(format!("{:02} - ", track));
            file_name.push(&artists);
            file_name.push(" - ");
            file_name.push(&title);
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

    pub fn create_dirs(&self, f: &mut impl FnMut(&DirCreation, io::Result<()>)) {
        for d in self.dir_creations.iter() {
            let r = d.execute();
            f(d, r);
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
