use std::{error, path::Path};

#[derive(Clone, Debug, Default, PartialEq)]
pub struct TagUpdate {
    pub track_number: Value<u16>,
    pub total_tracks: Value<u16>,
    pub disc_number: Value<u16>,
    pub total_discs: Value<u16>,
    pub artists: Value<Vec<String>>,
    pub album_artists: Value<Vec<String>>,
    pub album: Value<String>,
    pub title: Value<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Value<T> {
    Update(T),
    Remove,
    Unchanged,
}

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
}

impl TagUpdate {
    pub fn execute(&self, path: &Path) -> Result<(), Box<dyn error::Error>> {
        match path.extension().unwrap().to_str().unwrap() {
            "mp3" => {
                let tag = match id3::Tag::read_from_path(path) {
                    Ok(mut tag) => {
                        match &self.album_artists {
                            Value::Update(a) => tag.set_album_artist(a.join("\u{0}")),
                            Value::Remove => tag.remove_album_artist(),
                            Value::Unchanged => (),
                        }
                        match &self.album_artists {
                            Value::Update(a) => tag.set_artist(a.join("\u{0}")),
                            Value::Remove => tag.remove_artist(),
                            Value::Unchanged => (),
                        }
                        match &self.album {
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

                        tag
                    }
                    Err(_) => id3::Tag::default(),
                };

                tag.write_to_path(path, id3::Version::Id3v24)?;
            }
            "m4a" | "m4b" | "m4p" | "m4v" => {
                let tag = match mp4ameta::Tag::read_from_path(path) {
                    Ok(mut tag) => {
                        match &self.album_artists {
                            Value::Update(a) => tag.set_album_artists(a.clone()),
                            Value::Remove => tag.remove_album_artists(),
                            Value::Unchanged => (),
                        }
                        match &self.artists {
                            Value::Update(a) => tag.set_artists(a.clone()),
                            Value::Remove => tag.remove_artists(),
                            Value::Unchanged => (),
                        }
                        match &self.album {
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
