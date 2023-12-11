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
        match path.extension().unwrap().to_str().unwrap() {
            "mp3" => {
                if let Some(meta) = Self::read_mp3(path) {
                    return meta;
                }
            }
            "m4a" => {
                if let Some(meta) = Self::read_mp4(path) {
                    return meta;
                }
            }
            "flac" => {
                if let Some(meta) = Self::read_flac(path) {
                    return meta;
                }
            }
            _ => (),
        }

        Self::default()
    }

    fn read_mp3(path: &Path) -> Option<Self> {
        let tag = id3::Tag::read_from_path(path).ok()?;

        Some(Self {
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

    fn read_mp4(path: &Path) -> Option<Self> {
        let mut tag = mp4ameta::Tag::read_from_path(path).ok()?;
        Some(Self {
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

    fn read_flac(path: &Path) -> Option<Self> {
        let tag = metaflac::Tag::read_from_path(path).ok()?;
        let vorbis = tag.vorbis_comments()?;

        Some(Self {
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

#[inline]
pub fn zero_none(n: Option<u16>) -> Option<u16> {
    n.and_then(|n| match n {
        0 => None,
        _ => Some(n),
    })
}
