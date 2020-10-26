use std::ffi::{OsStr, OsString};
use std::iter::Iterator;
use std::path::PathBuf;
use std::{error, fs, io};
use walkdir::WalkDir;

const MUSIC_FILE_EXTENSIONS: [&str; 5] = ["m4a", "mp3", "m4b", "m4p", "m4v"];

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Artist {
    pub name: String,
    pub albums: Vec<Album>,
    pub singles: Vec<usize>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Album {
    pub name: String,
    pub songs: Vec<usize>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Song {
    pub track: Option<u16>,
    pub total_tracks: Option<u16>,
    pub disc: Option<u16>,
    pub total_discs: Option<u16>,
    pub artist: Option<String>,
    pub title: Option<String>,
    pub path: PathBuf,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Metadata {
    pub track: Option<u16>,
    pub total_tracks: Option<u16>,
    pub disc: Option<u16>,
    pub total_discs: Option<u16>,
    pub artist: Option<String>,
    pub album_artist: Option<String>,
    pub album: Option<String>,
    pub title: Option<String>,
}

impl Metadata {
    pub fn read_from(path: &PathBuf) -> Self {
        match path.extension().unwrap().to_str().unwrap() {
            "mp3" => {
                if let Ok(tag) = id3::Tag::read_from_path(&path) {
                    return Self {
                        track: zero_none(tag.track().map(|u| u as u16)),
                        total_tracks: zero_none(tag.total_tracks().map(|u| u as u16)),
                        disc: zero_none(tag.disc().map(|u| u as u16)),
                        total_discs: zero_none(tag.total_discs().map(|u| u as u16)),
                        artist: tag.artist().map(|s| s.to_string()),
                        album_artist: tag.album_artist().map(|s| s.to_string()),
                        album: tag.album().map(|s| s.to_string()),
                        title: tag.title().map(|s| s.to_string()),
                    };
                }
            }
            "m4a" | "m4b" | "m4p" | "m4v" => {
                if let Ok(tag) = mp4ameta::Tag::read_from_path(&path) {
                    return Self {
                        track: tag.track_number(),
                        total_tracks: tag.total_tracks(),
                        disc: tag.disc_number(),
                        total_discs: tag.total_discs(),
                        artist: tag.artist().map(|s| s.to_string()),
                        album_artist: tag.album_artist().map(|s| s.to_string()),
                        album: tag.album().map(|s| s.to_string()),
                        title: tag.title().map(|s| s.to_string()),
                    };
                }
            }
            _ => (),
        }

        Self::default()
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct DirCreation {
    pub path: PathBuf,
}

impl DirCreation {
    pub fn execute(&self) -> Result<(), io::Error> {
        std::fs::create_dir(&self.path)
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct FileOperation {
    pub old: PathBuf,
    pub new: Option<PathBuf>,
    pub tag_update: Option<TagUpdate>,
}

impl FileOperation {
    pub fn execute(&self, op_type: FileOpType) -> Result<(), Box<dyn error::Error>> {
        if let Some(new) = &self.new {
            match op_type {
                FileOpType::Copy => fs::copy(&self.old, new).map(|_| ())?,
                FileOpType::Move => fs::rename(&self.old, new)?,
            };
        }

        if let Some(u) = &self.tag_update {
            match &self.new {
                Some(n) => u.execute(n)?,
                None => u.execute(&self.old)?,
            }
        }

        Ok(())
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

#[derive(Clone, Debug, Default, PartialEq)]
pub struct TagUpdate {
    pub meta: Metadata,
}

impl TagUpdate {
    pub fn execute(&self, path: &PathBuf) -> Result<(), Box<dyn error::Error>> {
        match path.extension().unwrap().to_str().unwrap() {
            "mp3" => {
                let tag = match id3::Tag::read_from_path(path) {
                    Ok(mut tag) => {
                        if let Some(a) = &self.meta.artist {
                            match a.is_empty() {
                                true => tag.remove_artist(),
                                false => tag.set_artist(a),
                            }
                        }
                        if let Some(a) = &self.meta.album_artist {
                            match a.is_empty() {
                                true => tag.remove_album_artist(),
                                false => tag.set_album_artist(a),
                            }
                        }
                        if let Some(a) = &self.meta.album {
                            match a.is_empty() {
                                true => tag.remove_album(),
                                false => tag.set_album(a),
                            }
                        }
                        if let Some(t) = &self.meta.title {
                            match t.is_empty() {
                                true => tag.remove_title(),
                                false => tag.set_title(t),
                            }
                        }
                        if let Some(t) = self.meta.track {
                            match t == 0 {
                                true => tag.remove_track(),
                                false => tag.set_track(t as u32),
                            }
                        }
                        if let Some(t) = self.meta.total_tracks {
                            match t == 0 {
                                true => tag.remove_total_tracks(),
                                false => tag.set_total_tracks(t as u32),
                            }
                        }
                        if let Some(t) = self.meta.disc {
                            match t == 0 {
                                true => tag.remove_disc(),
                                false => tag.set_disc(t as u32),
                            }
                        }
                        if let Some(t) = self.meta.total_discs {
                            match t == 0 {
                                true => tag.remove_total_discs(),
                                false => tag.set_total_discs(t as u32),
                            }
                        }

                        tag
                    }
                    Err(_) => id3::Tag::default(),
                };

                tag.write_to_path(path, id3::Version::Id3v24)?;
            }
            "m4a" | "m4b" | "m4p" | "m4v" => {
                let tag = match mp4ameta::Tag::read_from_path(path) {
                    Ok(mut tag) => {
                        if let Some(a) = &self.meta.artist {
                            match a.is_empty() {
                                true => tag.remove_artists(),
                                false => tag.set_artist(a),
                            }
                        }
                        if let Some(a) = &self.meta.album_artist {
                            match a.is_empty() {
                                true => tag.remove_album_artists(),
                                false => tag.set_album_artist(a),
                            }
                        }
                        if let Some(a) = &self.meta.album {
                            match a.is_empty() {
                                true => tag.remove_album(),
                                false => tag.set_album(a),
                            }
                        }
                        if let Some(t) = &self.meta.title {
                            match t.is_empty() {
                                true => tag.remove_title(),
                                false => tag.set_title(t),
                            }
                        }
                        if let Some(t) = self.meta.track {
                            match t == 0 {
                                true => tag.remove_track_number(),
                                false => tag.set_track_number(t),
                            }
                        }
                        if let Some(t) = self.meta.total_tracks {
                            match t == 0 {
                                true => tag.remove_total_tracks(),
                                false => tag.set_total_tracks(t),
                            }
                        }
                        if let Some(t) = self.meta.disc {
                            match t == 0 {
                                true => tag.remove_disc_number(),
                                false => tag.set_disc_number(t),
                            }
                        }
                        if let Some(t) = self.meta.total_discs {
                            match t == 0 {
                                true => tag.remove_total_discs(),
                                false => tag.set_total_discs(t),
                            }
                        }

                        tag
                    }
                    Err(_) => mp4ameta::Tag::default(),
                };

                tag.write_to_path(path)?;
            }
            _ => (),
        }

        Ok(())
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
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
                path: p,
            };

            self.index.songs.push(song);

            let artist = if let Some(a) = &m.album_artist {
                a.clone()
            } else if let Some(a) = &m.artist {
                a.clone()
            } else {
                self.index.unknown.push(song_index);
                return Some(m);
            };

            let single_album_name = m.title.as_ref().map(|t| format!("{} - single", &t));
            let is_single = match (&m.album, &single_album_name) {
                (None, _) => true,
                (Some(al_name), Some(s_al_name)) => {
                    al_name.eq_ignore_ascii_case(s_al_name) || al_name.is_empty()
                }
                _ => false,
            };

            for ar in &mut self.index.artists {
                if ar.name == artist {
                    if is_single {
                        ar.singles.push(song_index);
                    } else {
                        for al in &mut ar.albums {
                            if &al.name == m.album.opt_str() {
                                al.songs.push(song_index);
                                return Some(m);
                            }
                        }

                        ar.albums.push(Album {
                            // Has to be Some otherwise would be a single
                            name: m.album.clone().unwrap(),
                            songs: vec![song_index],
                        });
                    }
                    return Some(m);
                }
            }

            if is_single {
                self.index.artists.push(Artist {
                    name: artist,
                    singles: vec![song_index],
                    albums: Vec::new(),
                });
            } else {
                self.index.artists.push(Artist {
                    name: artist,
                    singles: Vec::new(),
                    albums: vec![Album {
                        // Has to be Some otherwise would be a single
                        name: m.album.clone().unwrap(),
                        songs: vec![song_index],
                    }],
                });
            }

            return Some(m);
        }

        None
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Changes {
    pub dir_creations: Vec<DirCreation>,
    pub file_operations: Vec<FileOperation>,
}

impl Changes {
    pub fn update(&mut self, path: &PathBuf, f: impl FnOnce(&mut FileOperation)) {
        match self.file_operations.iter_mut().find(|f| &f.old == path) {
            Some(fo) => f(fo),
            None => {
                let mut fo = FileOperation {
                    old: path.clone(),
                    ..Default::default()
                };

                f(&mut fo);

                self.file_operations.push(fo);
            }
        }
    }

    pub fn update_tag(&mut self, path: &PathBuf, f: impl FnOnce(&mut TagUpdate)) {
        self.update(path, |fo| match &mut fo.tag_update {
            Some(tu) => f(tu),
            None => {
                let mut tu = TagUpdate::default();

                f(&mut tu);

                fo.tag_update = Some(tu);
            }
        });
    }

    pub fn check_inconsitent_artists(
        &mut self,
        index: &MusicIndex,
        f: fn(&MusicIndex, &Artist, &Artist) -> Option<String>,
    ) {
        let mut offset = 1;
        for ar1 in index.artists.iter() {
            for ar2 in index.artists.iter().skip(offset) {
                if ar1.name.eq_ignore_ascii_case(&ar2.name) {
                    if let Some(name) = f(index, ar1, ar2) {
                        if ar1.name != name {
                            for album in ar1.albums.iter() {
                                for song in album.songs.iter().map(|&si| &index.songs[si]) {
                                    self.update_tag(&song.path, |tu| {
                                        tu.meta.artist = Some(name.clone())
                                    });
                                }
                            }
                        }

                        if ar2.name != name {
                            for album in ar2.albums.iter() {
                                for song in album.songs.iter().map(|&si| &index.songs[si]) {
                                    self.update_tag(&song.path, |tu| {
                                        tu.meta.artist = Some(name.clone())
                                    });
                                }
                            }
                        }
                    }
                }
            }
            offset += 1;
        }
    }

    pub fn check_inconsitent_albums(
        &mut self,
        index: &MusicIndex,
        f: fn(&MusicIndex, &Artist, &Album, &Album) -> Option<String>,
    ) {
        for ar in index.artists.iter() {
            let mut offset = 1;
            for al1 in ar.albums.iter() {
                for al2 in ar.albums.iter().skip(offset) {
                    if al1.name.eq_ignore_ascii_case(&al2.name) {
                        if let Some(name) = f(index, ar, al1, al2) {
                            if al1.name != name {
                                for song in al1.songs.iter().map(|&si| &index.songs[si]) {
                                    self.update_tag(&song.path, |tu| {
                                        tu.meta.album = Some(name.clone())
                                    });
                                }
                            }

                            if al2.name != name {
                                for song in al2.songs.iter().map(|&si| &index.songs[si]) {
                                    self.update_tag(&song.path, |tu| {
                                        tu.meta.album = Some(name.clone())
                                    });
                                }
                            }
                        }
                    }
                }
                offset += 1;
            }
        }
    }

    pub fn check_inconsitent_total_tracks(
        &mut self,
        index: &MusicIndex,
        f: fn(&Artist, &Album, Vec<(Vec<&Song>, Option<u16>)>) -> Option<u16>,
    ) {
        for ar in index.artists.iter() {
            for al in ar.albums.iter() {
                let mut total_tracks: Vec<(Vec<&Song>, Option<u16>)> = Vec::new();

                'songs: for s in al.songs.iter().map(|&si| &index.songs[si]) {
                    for (songs, tt) in total_tracks.iter_mut() {
                        if *tt == s.total_tracks {
                            songs.push(s);
                            continue 'songs;
                        }
                    }

                    total_tracks.push((vec![s], s.total_tracks));
                }

                if total_tracks.len() > 1 {
                    if let Some(t) = f(ar, al, total_tracks) {
                        for song in al.songs.iter().map(|&si| &index.songs[si]) {
                            if song.total_tracks != zero_none(Some(t)) {
                                self.update_tag(&song.path, |tu| tu.meta.total_tracks = Some(t));
                            }
                        }
                    }
                }
            }
        }
    }

    pub fn check_inconsitent_total_discs(
        &mut self,
        index: &MusicIndex,
        f: fn(&Artist, &Album, Vec<(Vec<&Song>, Option<u16>)>) -> Option<u16>,
    ) {
        for ar in index.artists.iter() {
            for al in ar.albums.iter() {
                let mut total_discs: Vec<(Vec<&Song>, Option<u16>)> = Vec::new();

                'songs: for s in al.songs.iter().map(|&si| &index.songs[si]) {
                    for (songs, tt) in total_discs.iter_mut() {
                        if *tt == s.total_discs {
                            songs.push(s);
                            continue 'songs;
                        }
                    }

                    total_discs.push((vec![s], s.total_discs));
                }

                if total_discs.len() > 1 {
                    if let Some(t) = f(ar, al, total_discs) {
                        for song in al.songs.iter().map(|&si| &index.songs[si]) {
                            if song.total_discs != zero_none(Some(t)) {
                                self.update_tag(&song.path, |tu| tu.meta.total_discs = Some(t));
                            }
                        }
                    }
                }
            }
        }
    }

    pub fn file_system(&mut self, index: &MusicIndex, output_dir: &PathBuf) {
        if !output_dir.exists() {
            self.dir_creations.push(DirCreation {
                path: output_dir.clone(),
            })
        }

        for ar in index.artists.iter() {
            let ar_dir = output_dir.join(valid_os_string(&ar.name));
            if !ar_dir.exists() {
                self.dir_creations.push(DirCreation {
                    path: ar_dir.clone(),
                });
            }

            for song in ar.singles.iter().map(|&si| &index.songs[si]) {
                let extension = song.path.extension().unwrap();
                let mut file_name = OsString::with_capacity(
                    4 + song.artist.len() + song.title.len() + extension.len(),
                );

                file_name.push(valid_os_string(song.artist.opt_str()));
                file_name.push(" - ");
                file_name.push(valid_os_string(song.title.opt_str()));
                file_name.push(".");
                file_name.push(extension);

                let new_file = ar_dir.join(file_name);
                if new_file != song.path {
                    self.update(&song.path, |fo| fo.new = Some(new_file))
                }
            }

            for al in ar.albums.iter() {
                let al_dir = ar_dir.join(valid_os_string(&al.name));

                if !al_dir.exists() {
                    self.dir_creations.push(DirCreation {
                        path: al_dir.clone(),
                    });
                }

                for song in al.songs.iter().map(|&si| &index.songs[si]) {
                    let extension = song.path.extension().unwrap();
                    let mut file_name = OsString::with_capacity(
                        9 + song.artist.len() + song.title.len() + extension.len(),
                    );

                    let track = match song.track {
                        Some(n) => n,
                        _ => 0,
                    };

                    file_name.push(format!("{:02} - ", track));
                    file_name.push(valid_os_string(song.artist.opt_str()));
                    file_name.push(" - ");
                    file_name.push(valid_os_string(song.title.opt_str()));
                    file_name.push(".");
                    file_name.push(extension);

                    let new_file = al_dir.join(file_name);
                    if new_file != song.path {
                        self.update(&song.path, |fo| fo.new = Some(new_file))
                    }
                }
            }
        }

        if !index.unknown.is_empty() {
            let unknown_dir = output_dir.join("unknown");
            if !unknown_dir.exists() {
                self.dir_creations.push(DirCreation {
                    path: unknown_dir.clone(),
                });
            }
            for si in &index.unknown {
                let song = &index.songs[*si];
                let new_file = unknown_dir.join(song.path.file_name().unwrap());

                self.update(&song.path, |fo| fo.new = Some(new_file));
            }
        }
    }

    pub fn write(&self, op_type: FileOpType) -> Vec<Box<dyn error::Error>> {
        let mut errors: Vec<Box<dyn error::Error>> = Vec::new();

        for d in &self.dir_creations {
            if let Err(e) = d.execute() {
                errors.push(Box::new(e));
            }
        }

        errors
    }

    pub fn dir_creation_iter(&self) -> DirCreationIter {
        DirCreationIter::from(self)
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
    iter: Box<dyn Iterator<Item = FileOperation<'a>> + 'a>,
    op_type: FileOpType,
}

impl<'a, 'b> FileOperationIter<'a> {
    pub fn from(index: &'a MusicIndex, op_type: FileOpType) -> Self {
        Self {
            iter: Box::new(
                index
                    .songs
                    .iter()
                    .filter(|s| s.new_file.is_some())
                    .map(|s| FileOperation {
                        old: s.current_file.as_ref(),
                        new: s.new_file.as_ref().unwrap(),
                    }),
            ),
            op_type,
        }
    }
}

impl<'a> Iterator for FileOperationIter<'a> {
    type Item = (&'a FileOperation, Result<(), Box<dyn error::Error>>);

    fn next(&mut self) -> Option<Self::Item> {
        let f = self.iter.next()?;
        let r = f.execute(self.op_type);

        Some((f, r))
    }
}

pub trait OptionAsStr {
    fn opt_str(&self) -> &str;
}

impl OptionAsStr for Option<String> {
    fn opt_str(&self) -> &str {
        match self {
            Some(s) => s.as_str(),
            _ => "",
        }
    }
}

impl OptionAsStr for Option<&String> {
    fn opt_str(&self) -> &str {
        match self {
            Some(s) => s.as_str(),
            _ => "",
        }
    }
}

pub trait OptionLen {
    fn len(&self) -> usize;
}

impl OptionLen for Option<String> {
    fn len(&self) -> usize {
        match self {
            Some(s) => s.len(),
            _ => 0,
        }
    }
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

#[inline]
fn is_music_extension(s: &OsStr) -> bool {
    for e in &MUSIC_FILE_EXTENSIONS {
        if s.eq(*e) {
            return true;
        }
    }

    false
}

#[inline]
fn zero_none(n: Option<u16>) -> Option<u16> {
    match n {
        Some(n) => match n == 0 {
            true => None,
            false => Some(n),
        },
        None => None,
    }
}
