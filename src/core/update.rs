use std::{error, path::Path};

use id3::frame::Picture;
use id3::frame::PictureType as Id3PictureType;
use metaflac::block::PictureType as FlacPictureType;
use mp4ameta::Img;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct TagUpdate {
    pub track_number: Value<u16>,
    pub total_tracks: Value<u16>,
    pub disc_number: Value<u16>,
    pub total_discs: Value<u16>,
    pub artists: Value<Vec<String>>,
    pub release_artists: Value<Vec<String>>,
    pub release: Value<String>,
    pub title: Value<String>,
    pub artwork: Value<Vec<u8>>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Value<T> {
    Update(T),
    Remove,
    Unchanged,
}

impl<T: Copy> Copy for Value<T> {}

impl<T> Default for Value<T> {
    fn default() -> Self {
        Self::Unchanged
    }
}

impl<T> Value<T> {
    pub fn value(&self) -> Option<&T> {
        match self {
            Self::Update(v) => Some(v),
            _ => None,
        }
    }

    pub fn is_update(&self) -> bool {
        matches!(self, Self::Update(_))
    }

    pub fn is_remove(&self) -> bool {
        matches!(self, Self::Remove)
    }

    pub fn is_unchanged(&self) -> bool {
        matches!(self, Self::Unchanged)
    }
}

impl TagUpdate {
    pub fn execute(&self, path: &Path) -> Result<(), Box<dyn error::Error>> {
        match path.extension().unwrap().to_str().unwrap() {
            "mp3" => self.write_mp3(path)?,
            "m4a" => self.write_mp4(path)?,
            "flac" => self.write_flac(path)?,
            _ => (),
        }

        Ok(())
    }

    fn write_mp3(&self, path: &Path) -> Result<(), Box<dyn error::Error>> {
        let tag = match id3::Tag::read_from_path(path) {
            Ok(mut tag) => {
                match &self.release_artists {
                    Value::Update(a) => tag.set_album_artist(a.join("\u{0}")),
                    Value::Remove => tag.remove_album_artist(),
                    Value::Unchanged => (),
                }
                match &self.release_artists {
                    Value::Update(a) => tag.set_artist(a.join("\u{0}")),
                    Value::Remove => tag.remove_artist(),
                    Value::Unchanged => (),
                }
                match &self.release {
                    Value::Update(a) => tag.set_album(a),
                    Value::Remove => tag.remove_album(),
                    Value::Unchanged => (),
                }
                match &self.title {
                    Value::Update(t) => tag.set_title(t),
                    Value::Remove => tag.remove_title(),
                    Value::Unchanged => (),
                }
                match &self.track_number {
                    Value::Update(t) => tag.set_track(*t as u32),
                    Value::Remove => tag.remove_track(),
                    Value::Unchanged => (),
                }
                match &self.total_tracks {
                    Value::Update(t) => tag.set_total_tracks(*t as u32),
                    Value::Remove => tag.remove_total_tracks(),
                    Value::Unchanged => (),
                }
                match &self.disc_number {
                    Value::Update(d) => tag.set_disc(*d as u32),
                    Value::Remove => tag.remove_disc(),
                    Value::Unchanged => (),
                }
                match &self.total_discs {
                    Value::Update(d) => tag.set_total_discs(*d as u32),
                    Value::Remove => tag.remove_total_discs(),
                    Value::Unchanged => (),
                }
                match &self.artwork {
                    Value::Update(d) => {
                        tag.remove_all_pictures();
                        tag.add_picture(Picture {
                            mime_type: "image/png".to_string(),
                            picture_type: Id3PictureType::CoverFront,
                            description: "".to_string(),
                            data: d.clone(),
                        })
                    }
                    Value::Remove => tag.remove_all_pictures(),
                    Value::Unchanged => (),
                }

                tag
            }
            Err(_) => id3::Tag::default(),
        };

        tag.write_to_path(path, id3::Version::Id3v24)?;

        Ok(())
    }

    fn write_mp4(&self, path: &Path) -> Result<(), Box<dyn error::Error>> {
        let tag = match mp4ameta::Tag::read_from_path(path) {
            Ok(mut tag) => {
                match &self.release_artists {
                    Value::Update(a) => tag.set_album_artists(a.clone()),
                    Value::Remove => tag.remove_album_artists(),
                    Value::Unchanged => (),
                }
                match &self.artists {
                    Value::Update(a) => tag.set_artists(a.clone()),
                    Value::Remove => tag.remove_artists(),
                    Value::Unchanged => (),
                }
                match &self.release {
                    Value::Update(a) => tag.set_album(a),
                    Value::Remove => tag.remove_album(),
                    Value::Unchanged => (),
                }
                match &self.title {
                    Value::Update(t) => tag.set_title(t),
                    Value::Remove => tag.remove_title(),
                    Value::Unchanged => (),
                }
                match &self.track_number {
                    Value::Update(t) => tag.set_track_number(*t),
                    Value::Remove => tag.remove_track_number(),
                    Value::Unchanged => (),
                }
                match &self.total_tracks {
                    Value::Update(t) => tag.set_total_tracks(*t),
                    Value::Remove => tag.remove_total_tracks(),
                    Value::Unchanged => (),
                }
                match &self.disc_number {
                    Value::Update(d) => tag.set_disc_number(*d),
                    Value::Remove => tag.remove_disc_number(),
                    Value::Unchanged => (),
                }
                match &self.total_discs {
                    Value::Update(d) => tag.set_total_discs(*d),
                    Value::Remove => tag.remove_total_discs(),
                    Value::Unchanged => (),
                }
                match &self.artwork {
                    Value::Update(d) => tag.set_artwork(Img::png(d.clone())),
                    Value::Remove => tag.remove_artworks(),
                    Value::Unchanged => (),
                }

                tag
            }
            Err(_) => mp4ameta::Tag::default(),
        };

        tag.write_to_path(path)?;

        Ok(())
    }

    fn write_flac(&self, path: &Path) -> Result<(), Box<dyn error::Error>> {
        let mut tag = match metaflac::Tag::read_from_path(path) {
            Ok(mut tag) => {
                let vorbis = tag.vorbis_comments_mut();

                match &self.release_artists {
                    Value::Update(a) => vorbis.set_album_artist(a.clone()),
                    Value::Remove => vorbis.remove_album_artist(),
                    Value::Unchanged => (),
                }
                match &self.artists {
                    Value::Update(a) => vorbis.set_artist(a.clone()),
                    Value::Remove => vorbis.remove_artist(),
                    Value::Unchanged => (),
                }
                match &self.release {
                    Value::Update(a) => vorbis.set_album(vec![a]),
                    Value::Remove => vorbis.remove_album(),
                    Value::Unchanged => (),
                }
                match &self.title {
                    Value::Update(t) => vorbis.set_title(vec![t]),
                    Value::Remove => vorbis.remove_title(),
                    Value::Unchanged => (),
                }
                match &self.track_number {
                    Value::Update(t) => vorbis.set_track(*t as u32),
                    Value::Remove => vorbis.remove_track(),
                    Value::Unchanged => (),
                }
                match &self.total_tracks {
                    Value::Update(t) => vorbis.set_total_tracks(*t as u32),
                    Value::Remove => vorbis.remove_total_tracks(),
                    Value::Unchanged => (),
                }
                match &self.disc_number {
                    Value::Update(d) => vorbis.set("DISCNUMBER", vec![d.to_string()]),
                    Value::Remove => vorbis.remove("DISCNUMBER"),
                    Value::Unchanged => (),
                }
                match &self.total_discs {
                    Value::Update(d) => vorbis.set("TOTALDISCS", vec![d.to_string()]),
                    Value::Remove => vorbis.remove("TOTALDISCS"),
                    Value::Unchanged => (),
                }
                match &self.artwork {
                    Value::Update(d) => {
                        tag.add_picture("image/png", FlacPictureType::CoverFront, d.clone())
                    }
                    Value::Remove => tag.remove_picture_type(FlacPictureType::CoverFront),
                    Value::Unchanged => (),
                }

                tag
            }
            Err(_) => metaflac::Tag::default(),
        };

        tag.write_to_path(path)?;

        Ok(())
    }
}
