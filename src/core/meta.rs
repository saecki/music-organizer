use std::ffi::OsStr;
use std::fmt::Display;
use std::fs::{File, OpenOptions, Permissions};
use std::path::{Path, PathBuf};

use anyhow::{bail, Context};
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

impl Song {
    pub fn write_metadata_to(&self, path: &Path, format: AudioFormat) -> anyhow::Result<()> {
        let mut file = OpenOptions::new().read(true).write(true).open(path)?;

        let image = (self.has_artwork)
            .then(|| Image::read_from(&self.path, self.format).context("error reading artwork"))
            .transpose()?;

        match format {
            AudioFormat::Flac => {
                self.write_flac(&mut file, image).context("error writing flac tag")
            }
            AudioFormat::M4a => self.write_mp4(&mut file, image).context("error writing m4a tag"),
            AudioFormat::Mp3 => self.write_mp3(&mut file, image).context("error writing mp3 tag"),
            AudioFormat::Opus => {
                self.write_opus(&mut file, image).context("error writing opus tag")
            }
        }
    }

    fn write_flac(&self, file: &mut File, image: Option<Image>) -> anyhow::Result<()> {
        let mut tag = metaflac::Tag::new();
        let vorbis = tag.vorbis_comments_mut();
        vorbis.set_track(self.track_number.unwrap_or(0).into());
        vorbis.set_total_tracks(self.total_tracks.unwrap_or(0).into());
        vorbis.set("DISCNUMBER", vec![self.disc_number.unwrap_or(0).to_string()]);
        vorbis.set("TOTALDISCS", vec![self.total_discs.unwrap_or(0).to_string()]);
        vorbis.set_artist(self.artists.clone());
        vorbis.set_album_artist(self.album_artists.clone());
        vorbis.set_album(vec![self.album.clone()]);
        vorbis.set_title(vec![self.title.clone()]);
        vorbis.set_genre(self.genres.clone());

        if let Some(image) = image {
            tag.add_picture(
                image.format.into_mime_type(),
                metaflac::block::PictureType::CoverFront,
                image.data,
            );
        }

        _ = file;
        todo!("metaflac `write_to` will just write a FLAC tag without moving/adjusting other data");
    }

    fn write_mp4(&self, file: &mut File, image: Option<Image>) -> anyhow::Result<()> {
        let mut tag = mp4ameta::Userdata::default();
        tag.set_track_number(self.track_number.unwrap_or(0));
        tag.set_total_tracks(self.total_tracks.unwrap_or(0));
        tag.set_disc_number(self.disc_number.unwrap_or(0));
        tag.set_total_discs(self.total_discs.unwrap_or(0));
        tag.set_artists(self.artists.clone());
        tag.set_album_artists(self.album_artists.clone());
        tag.set_album(self.album.clone());
        tag.set_title(self.title.clone());
        tag.set_genres(self.genres.clone());

        if let Some(image) = image {
            tag.set_artwork(mp4ameta::Img::new(image.format.mp4(), image.data));
        }

        tag.write_to(file)?;
        Ok(())
    }

    fn write_mp3(&self, file: &mut File, image: Option<Image>) -> anyhow::Result<()> {
        let mut tag = id3::Tag::new();
        tag.set_track(self.track_number.unwrap_or(0).into());
        tag.set_total_tracks(self.total_tracks.unwrap_or(0).into());
        tag.set_disc(self.disc_number.unwrap_or(0).into());
        tag.set_total_discs(self.total_discs.unwrap_or(0).into());
        tag.set_artist(self.artists.join("\0"));
        tag.set_album_artist(self.album_artists.join("\0"));
        tag.set_album(self.album.clone());
        tag.set_title(self.title.clone());
        tag.set_genre(self.genres.join("\0"));

        if let Some(image) = image {
            tag.add_frame(id3::frame::Picture {
                mime_type: image.format.into_mime_type(),
                picture_type: id3::frame::PictureType::CoverFront,
                description: String::new(),
                data: image.data,
            });
        }

        tag.write_to(file, id3::Version::Id3v24)?;
        Ok(())
    }

    fn write_opus(&self, file: &mut File, image: Option<Image>) -> anyhow::Result<()> {
        let mut tag = opusmeta::Tag::default();
        tag.add_one(opus::TRACKNUMBER, self.track_number.unwrap_or(0).to_string());
        tag.add_one(opus::TOTALTRACKS, self.total_tracks.unwrap_or(0).to_string());
        tag.add_one(opus::DISCNUMBER, self.disc_number.unwrap_or(0).to_string());
        tag.add_one(opus::TOTALDISCS, self.total_discs.unwrap_or(0).to_string());
        tag.add_many(opus::ARTIST, self.artists.clone());
        tag.add_many(opus::ALBUMARTIST, self.album_artists.clone());
        tag.add_one(opus::ALBUM, self.album.clone());
        tag.add_one(opus::TITLE, self.title.clone());
        tag.add_many(opus::GENRE, self.genres.clone());

        if let Some(image) = image {
            tag.add_picture(&opusmeta::picture::Picture {
                mime_type: image.format.into_mime_type(),
                picture_type: opusmeta::picture::PictureType::CoverFront,
                description: String::new(),
                data: image.data,
            })?;
        }

        tag.write_to(file)?;
        Ok(())
    }
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

    fn read_mp3(file: &File) -> anyhow::Result<Self> {
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

    fn read_opus(file: &File) -> anyhow::Result<Self> {
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
            album: tag.get_one(&opus::ALBUM).map(|s| s.clone()),
            title: tag.get_one(&opus::TITLE).map(|s| s.clone()),
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
    format: ImageFormat,
    data: Vec<u8>,
}

impl Image {
    pub fn read_from(path: &Path, format: AudioFormat) -> anyhow::Result<Self> {
        match format {
            AudioFormat::Flac => {
                let tag = metaflac::Tag::read_from_path(path)?;
                let Some(picture) = tag.pictures().next() else {
                    bail!("expected picture in flac file");
                };
                Ok(Image {
                    format: ImageFormat::Mime(picture.mime_type.clone()),
                    data: picture.data.clone(),
                })
            }
            AudioFormat::M4a => {
                let mut tag = mp4ameta::Tag::read_from_path(path)?;
                let Some(img) = tag.take_artwork() else { bail!("expected picture in m4a file") };
                Ok(Image { format: ImageFormat::Mp4(img.fmt), data: img.data })
            }
            AudioFormat::Mp3 => {
                let tag = id3::Tag::read_from_path(path)?;
                let Some(picture) = tag.pictures().next() else {
                    bail!("expected picture in mp3 file")
                };
                Ok(Image {
                    format: ImageFormat::Mime(picture.mime_type.clone()),
                    data: picture.data.clone(),
                })
            }
            AudioFormat::Opus => {
                let tag = opusmeta::Tag::read_from_path(path)?;
                let Some(picture) = tag.iter_pictures().and_then(|mut iter| iter.next()) else {
                    bail!("expected picture in opus file")
                };
                let picture = picture?;
                Ok(Image {
                    format: ImageFormat::Mime(picture.mime_type.clone()),
                    data: picture.data.clone(),
                })
            }
        }
    }
}

enum ImageFormat {
    Mime(String),
    Mp4(mp4ameta::ImgFmt),
}

impl ImageFormat {
    fn into_mime_type(self) -> String {
        match self {
            ImageFormat::Mime(mime) => mime,
            ImageFormat::Mp4(mp4ameta::ImgFmt::Bmp) => "image/bmp".into(),
            ImageFormat::Mp4(mp4ameta::ImgFmt::Jpeg) => "image/jpeg".into(),
            ImageFormat::Mp4(mp4ameta::ImgFmt::Png) => "image/png".into(),
        }
    }

    fn mp4(&self) -> mp4ameta::ImgFmt {
        match self {
            ImageFormat::Mime(mime) => match mime.as_str() {
                "image/bmp" => mp4ameta::ImgFmt::Bmp,
                "image/jpeg" | "image/jpg" => mp4ameta::ImgFmt::Jpeg,
                "image/png" => mp4ameta::ImgFmt::Png,
                // Default to png, the specific image format is ignored often
                // anyway by mp4 parsers.
                _ => mp4ameta::ImgFmt::Png,
            },
            ImageFormat::Mp4(fmt) => *fmt,
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

mod opus {
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
