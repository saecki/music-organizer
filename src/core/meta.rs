use std::error::Error;
use std::fmt::Write;
use std::fs::{File, Permissions};
use std::path::{Path, PathBuf};

use id3::TagLike;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ReleaseArtists<'a> {
    pub names: &'a [String],
    pub releases: Vec<Release<'a>>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Release<'a> {
    pub name: &'a str,
    pub songs: Vec<&'a Song>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Song {
    pub path: PathBuf,
    pub mode: Option<Mode>,
    pub track_number: Option<u16>,
    pub total_tracks: Option<u16>,
    pub disc_number: Option<u16>,
    pub total_discs: Option<u16>,
    pub release_artists: Vec<String>,
    pub artists: Vec<String>,
    pub release: String,
    pub title: String,
    pub has_artwork: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Metadata {
    pub mode: Option<Mode>,
    pub track_number: Option<u16>,
    pub total_tracks: Option<u16>,
    pub disc_number: Option<u16>,
    pub total_discs: Option<u16>,
    pub artists: Vec<String>,
    pub release_artists: Vec<String>,
    pub release: Option<String>,
    pub title: Option<String>,
    pub has_artwork: bool,
}

impl Metadata {
    pub fn read_from(path: &Path) -> Self {
        let Ok(mut file) = File::open(path) else { return Self::default() };
        match path.extension().unwrap().to_str().unwrap() {
            "mp3" => {
                if let Some(meta) = Self::read_mp3(&file) {
                    return meta;
                }
            }
            "m4a" => {
                if let Some(meta) = Self::read_mp4(&mut file) {
                    return meta;
                }
            }
            "flac" => {
                if let Some(meta) = Self::read_flac(&mut file) {
                    return meta;
                }
            }
            _ => (),
        }

        Self::default()
    }

    fn read_mp3(file: &File) -> Option<Self> {
        let tag = id3::Tag::read_from(file).ok()?;

        Some(Self {
            mode: Mode::read(file),
            track_number: zero_none(tag.track().map(|u| u as u16)),
            total_tracks: zero_none(tag.total_tracks().map(|u| u as u16)),
            disc_number: zero_none(tag.disc().map(|u| u as u16)),
            total_discs: zero_none(tag.total_discs().map(|u| u as u16)),
            artists: tag
                .artist()
                .map(|s| s.split('\u{0}').map(|s| s.to_string()).collect())
                .unwrap_or_default(),
            release_artists: tag
                .album_artist()
                .map(|s| s.split('\u{0}').map(|s| s.to_string()).collect())
                .unwrap_or_default(),
            release: tag.album().map(|s| s.to_string()),
            title: tag.title().map(|s| s.to_string()),
            has_artwork: tag.pictures().count() > 0,
        })
    }

    fn read_mp4(file: &mut File) -> Option<Self> {
        let mut tag = mp4ameta::Tag::read_from(file).ok()?;
        Some(Self {
            mode: Mode::read(file),
            track_number: tag.track_number(),
            total_tracks: tag.total_tracks(),
            disc_number: tag.disc_number(),
            total_discs: tag.total_discs(),
            artists: tag.take_artists().collect(),
            release_artists: tag.take_album_artists().collect(),
            release: tag.take_album(),
            title: tag.take_title(),
            has_artwork: tag.artwork().is_some(),
        })
    }

    fn read_flac(file: &mut File) -> Option<Self> {
        let tag = metaflac::Tag::read_from(file).ok()?;
        let vorbis = tag.vorbis_comments()?;

        Some(Self {
            mode: Mode::read(file),
            track_number: zero_none(vorbis.track().map(|u| u as u16)),
            total_tracks: zero_none(vorbis.total_tracks().map(|u| u as u16)),
            disc_number: zero_none(vorbis.get("DISCNUMBER").and_then(|d| d[0].parse().ok())),
            total_discs: zero_none(vorbis.get("TOTALDISCS").and_then(|d| d[0].parse().ok())),
            artists: vorbis.artist().map_or_else(Vec::new, |v| v.to_owned()),
            release_artists: vorbis.album_artist().map_or_else(Vec::new, |v| v.to_owned()),
            release: vorbis.album().map(|v| v[0].clone()),
            title: vorbis.title().map(|v| v[0].clone()),
            has_artwork: tag.pictures().count() > 0,
        })
    }

    pub fn release_artists(&self) -> Option<&[String]> {
        if !self.release_artists.is_empty() {
            Some(&self.release_artists)
        } else if !self.artists.is_empty() {
            Some(&self.artists)
        } else {
            None
        }
    }

    pub fn song_artists(&self) -> Option<&[String]> {
        if !self.artists.is_empty() {
            Some(&self.artists)
        } else if !self.release_artists.is_empty() {
            Some(&self.release_artists)
        } else {
            None
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Mode(pub u32);

impl std::fmt::Display for Mode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        fn write_char_or_dash(
            f: &mut std::fmt::Formatter<'_>,
            val: u32,
            char: char,
        ) -> std::fmt::Result {
            if val == 0 {
                f.write_char('-')
            } else {
                f.write_char(char)
            }
        }
        fn write_permissions(
            f: &mut std::fmt::Formatter<'_>,
            mode: u32,
            offset: u32,
        ) -> std::fmt::Result {
            write_char_or_dash(f, mode & (0o4 << offset), 'r')?;
            write_char_or_dash(f, mode & (0o2 << offset), 'w')?;
            write_char_or_dash(f, mode & (0o1 << offset), 'x')?;
            Ok(())
        }
        write_permissions(f, self.0, 6)?;
        write_permissions(f, self.0, 3)?;
        write_permissions(f, self.0, 0)?;
        Ok(())
    }
}

impl Mode {
    pub fn read(file: &File) -> Option<Mode> {
        use std::os::unix::fs::MetadataExt;

        let meta = file.metadata().ok()?;
        Some(Mode(meta.mode()))
    }

    pub fn write(&self, path: &Path) -> Result<(), Box<dyn Error>> {
        use std::os::unix::fs::PermissionsExt;

        let file = File::open(path)?;
        file.set_permissions(Permissions::from_mode(self.0))?;
        Ok(())
    }

    pub fn permissions(&self) -> u32 {
        self.0 & 0o777
    }

    pub fn with_permissions(&self, permissions: u32) -> Self {
        Self((self.0 & !0o777) | (permissions & 0o777))
    }
}

#[inline]
pub fn zero_none(n: Option<u16>) -> Option<u16> {
    n.and_then(|n| match n {
        0 => None,
        _ => Some(n),
    })
}
