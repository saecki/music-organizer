use std::ffi::OsStr;
use std::fmt::Display;
use std::fs::{File, Permissions};
use std::path::{Path, PathBuf};

use anyhow::bail;
use id3::TagLike;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ReleaseArtists<'a> {
    pub names: &'a [String],
    pub releases: Vec<Album<'a>>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Album<'a> {
    pub name: &'a str,
    pub songs: Vec<&'a Song>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AudioFormat {
    Flac,
    M4a,
    Mp3,
    Opus,
}

impl AudioFormat {
    pub fn from_extension(extension: &OsStr) -> Option<Self> {
        let str = extension.to_str()?;
        let fmt = match str {
            "m4a" => AudioFormat::M4a,
            "mp3" => AudioFormat::Mp3,
            "flac" => AudioFormat::Flac,
            "opus" => AudioFormat::Opus,
            _ => return None,
        };
        Some(fmt)
    }

    pub fn extension(&self) -> &'static str {
        match self {
            AudioFormat::Flac => "flac",
            AudioFormat::M4a => "m4a",
            AudioFormat::Mp3 => "mp3",
            AudioFormat::Opus => "opus",
        }
    }
}

impl Display for AudioFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.extension())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Song {
    pub format: AudioFormat,
    pub path: PathBuf,
    pub mode: Option<Mode>,
    pub track_number: Option<u16>,
    pub total_tracks: Option<u16>,
    pub disc_number: Option<u16>,
    pub total_discs: Option<u16>,
    pub album_artists: Vec<String>,
    pub artists: Vec<String>,
    pub album: String,
    pub title: String,
    pub genres: Vec<String>,
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
    pub album_artists: Vec<String>,
    pub album: Option<String>,
    pub title: Option<String>,
    pub genres: Vec<String>,
    pub has_artwork: bool,
}

impl Metadata {
    pub fn read_from(path: &Path, format: AudioFormat) -> anyhow::Result<Self> {
        let mut file = File::open(path)?;
        let mode = Mode::read(&file)?;
        let meta = match format {
            AudioFormat::Flac => Self::read_flac(&mut file),
            AudioFormat::M4a => Self::read_mp4(&mut file),
            AudioFormat::Mp3 => Self::read_mp3(&mut file),
            AudioFormat::Opus => Self::read_opus(&mut file),
        }?;
        Ok(Self { mode: Some(mode), ..meta })
    }

    fn read_flac(file: &mut File) -> anyhow::Result<Self> {
        let tag = metaflac::Tag::read_from(file)?;
        let Some(vorbis) = tag.vorbis_comments() else { return Ok(Self::default()) };

        Ok(Self {
            mode: None,
            track_number: zero_none(vorbis.track().map(|u| u as u16)),
            total_tracks: zero_none(vorbis.total_tracks().map(|u| u as u16)),
            disc_number: zero_none(vorbis.get("DISCNUMBER").and_then(|d| d[0].parse().ok())),
            total_discs: zero_none(vorbis.get("TOTALDISCS").and_then(|d| d[0].parse().ok())),
            artists: vorbis.artist().map_or_else(Vec::new, |v| v.clone()),
            album_artists: vorbis.album_artist().map_or_else(Vec::new, |v| v.clone()),
            album: vorbis.album().map(|v| v[0].clone()),
            title: vorbis.title().map(|v| v[0].clone()),
            genres: vorbis.genre().map_or_else(Vec::new, |v| v.clone()),
            has_artwork: tag.pictures().count() > 0,
        })
    }

    fn read_mp4(file: &mut File) -> anyhow::Result<Self> {
        let cfg = mp4ameta::ReadConfig {
            read_meta_items: true,
            read_image_data: false,
            read_chapter_list: false,
            read_chapter_track: false,
            read_audio_info: false,
            ..Default::default()
        };
        let mut tag = mp4ameta::Tag::read_with(file, &cfg)?;
        Ok(Self {
            mode: None,
            track_number: tag.track_number(),
            total_tracks: tag.total_tracks(),
            disc_number: tag.disc_number(),
            total_discs: tag.total_discs(),
            artists: tag.take_artists().collect(),
            album_artists: tag.take_album_artists().collect(),
            album: tag.take_album(),
            title: tag.take_title(),
            genres: tag.take_genres().collect(),
            has_artwork: tag.artwork().is_some(),
        })
    }

    fn read_mp3(file: &mut File) -> anyhow::Result<Self> {
        let tag = id3::Tag::read_from2(file)?;

        fn nul_separated(s: &str) -> Vec<String> {
            s.split('\0').map(|s| s.to_string()).collect()
        }

        Ok(Self {
            mode: None,
            track_number: zero_none(tag.track().map(|u| u as u16)),
            total_tracks: zero_none(tag.total_tracks().map(|u| u as u16)),
            disc_number: zero_none(tag.disc().map(|u| u as u16)),
            total_discs: zero_none(tag.total_discs().map(|u| u as u16)),
            artists: tag.artist().map(nul_separated).unwrap_or_default(),
            album_artists: tag.album_artist().map(nul_separated).unwrap_or_default(),
            album: tag.album().map(|s| s.to_string()),
            title: tag.title().map(|s| s.to_string()),
            genres: tag.genre().map(nul_separated).unwrap_or_default(),
            has_artwork: tag.pictures().count() > 0,
        })
    }

    fn read_opus(file: &mut File) -> anyhow::Result<Self> {
        let tag = opusmeta::Tag::read_from(file)?;
        Ok(Self {
            mode: None,
            track_number: tag.get_one(&opus::TRACKNUMBER).and_then(|s| s.parse().ok()),
            total_tracks: tag
                .get_one(&opus::TOTALTRACKS)
                .or_else(|| tag.get_one(&opus::TRACKTOTAL))
                .and_then(|s| s.parse().ok()),
            disc_number: tag.get_one(&opus::DISCNUMBER).and_then(|s| s.parse().ok()),
            total_discs: tag
                .get_one(&opus::TOTALDISCS)
                .or_else(|| tag.get_one(&opus::DISCTOTAL))
                .and_then(|s| s.parse().ok()),
            artists: tag.get(&opus::ARTIST).map_or_else(Vec::new, |v| v.clone()),
            album_artists: tag.get(&opus::ALBUMARTIST).map_or_else(Vec::new, |v| v.clone()),
            album: tag.get_one(&opus::ALBUM).cloned(),
            title: tag.get_one(&opus::TITLE).cloned(),
            genres: tag.get(&opus::GENRE).map_or_else(Vec::new, |v| v.clone()),
            has_artwork: tag.iter_pictures().and_then(|mut iter| iter.next()).is_some(),
        })
    }

    pub fn album_artists(&self) -> Option<&[String]> {
        if !self.album_artists.is_empty() {
            Some(&self.album_artists)
        } else if !self.artists.is_empty() {
            Some(&self.artists)
        } else {
            None
        }
    }

    pub fn song_artists(&self) -> Option<&[String]> {
        if !self.artists.is_empty() {
            Some(&self.artists)
        } else if !self.album_artists.is_empty() {
            Some(&self.album_artists)
        } else {
            None
        }
    }
}

pub struct Image {
    pub data: Vec<u8>,
}

impl Image {
    pub fn read_from(path: &Path, format: AudioFormat) -> anyhow::Result<Self> {
        match format {
            AudioFormat::Flac => {
                let tag = metaflac::Tag::read_from_path(path)?;
                let Some(picture) = tag.pictures().next() else {
                    bail!("expected picture in flac file");
                };
                Ok(Image { data: picture.data.clone() })
            }
            AudioFormat::M4a => {
                let mut tag = mp4ameta::Tag::read_from_path(path)?;
                let Some(img) = tag.take_artwork() else { bail!("expected picture in m4a file") };
                Ok(Image { data: img.data })
            }
            AudioFormat::Mp3 => {
                let tag = id3::Tag::read_from_path(path)?;
                let Some(picture) = tag.pictures().next() else {
                    bail!("expected picture in mp3 file")
                };
                Ok(Image { data: picture.data.clone() })
            }
            AudioFormat::Opus => {
                let tag = opusmeta::Tag::read_from_path(path)?;
                let Some(picture) = tag.iter_pictures().and_then(|mut iter| iter.next()) else {
                    bail!("expected picture in opus file")
                };
                let picture = picture?;
                Ok(Image { data: picture.data.clone() })
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Mode(pub u32);

impl std::fmt::Display for Mode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        fn write_permissions(
            f: &mut std::fmt::Formatter<'_>,
            mode: u32,
            offset: u32,
        ) -> std::fmt::Result {
            if mode & (0o4 << offset) == 0 {
                f.write_str("\x1b[90m-\x1b[0m")?;
            } else {
                f.write_str("\x1b[93mr\x1b[0m")?;
            }
            if mode & (0o2 << offset) == 0 {
                f.write_str("\x1b[90m-\x1b[0m")?;
            } else {
                f.write_str("\x1b[91mw\x1b[0m")?;
            }
            if mode & (0o1 << offset) == 0 {
                f.write_str("\x1b[90m-\x1b[0m")?;
            } else {
                f.write_str("\x1b[92mx\x1b[0m")?;
            }
            Ok(())
        }
        write_permissions(f, self.0, 6)?;
        write_permissions(f, self.0, 3)?;
        write_permissions(f, self.0, 0)?;
        Ok(())
    }
}

impl Mode {
    pub fn read(file: &File) -> std::io::Result<Mode> {
        use std::os::unix::fs::MetadataExt;

        let meta = file.metadata()?;
        Ok(Mode(meta.mode()))
    }

    pub fn write(&self, path: &Path) -> anyhow::Result<()> {
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

pub mod opus {
    use opusmeta::LowercaseString;

    macro_rules! key {
        ($name:ident = $str:literal) => {
            pub const $name: LowercaseString<'static> =
                LowercaseString::try_from_str($str).unwrap();
        };
    }

    key!(TRACKNUMBER = "tracknumber");
    key!(TOTALTRACKS = "totaltracks");
    key!(TRACKTOTAL = "tracktotal");
    key!(DISCNUMBER = "discnumber");
    key!(TOTALDISCS = "totaldiscs");
    key!(DISCTOTAL = "disctotal");
    key!(ARTIST = "artist");
    key!(ALBUMARTIST = "albumartist");
    key!(ALBUM = "album");
    key!(TITLE = "title");
    key!(GENRE = "genre");
}
