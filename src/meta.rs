use std::path::PathBuf;

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

    pub fn top_level_artist(&self) -> Option<&String> {
        if self.album_artist.is_some() {
            self.album_artist.as_ref()
        } else if self.artist.is_some() {
            self.artist.as_ref()
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
