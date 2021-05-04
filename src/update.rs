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

//impl<T> Value<T> {
//    fn value(&self) -> Option<&T> {
//        match self {
//            Self::Update(v) => Some(v),
//            _ => None,
//        }
//    }
//}

//impl TagUpdate {
//    pub fn execute(&self, path: &Path) -> Result<(), Box<dyn error::Error>> {
//        match path.extension().unwrap().to_str().unwrap() {
//            "mp3" => {
//                let tag = match id3::Tag::read_from_path(path) {
//                    Ok(mut tag) => {
//                        if let Some(a) = &self.artists {
//                            match a.is_empty() {
//                                true => tag.remove_artist(),
//                                false => tag.set_artist(a),
//                            }
//                        }
//                        if let Some(a) = &self.album_artists {
//                            match a.is_empty() {
//                                true => tag.remove_album_artist(),
//                                false => tag.set_album_artist(a),
//                            }
//                        }
//                        if let Some(a) = &self.album {
//                            match a.is_empty() {
//                                true => tag.remove_album(),
//                                false => tag.set_album(a),
//                            }
//                        }
//                        if let Some(t) = &self.title {
//                            match t.is_empty() {
//                                true => tag.remove_title(),
//                                false => tag.set_title(t),
//                            }
//                        }
//                        if let Some(t) = self.track_number {
//                            match t {
//                                0 => tag.remove_track(),
//                                _ => tag.set_track(t as u32),
//                            }
//                        }
//                        if let Some(t) = self.total_tracks {
//                            match t {
//                                0 => tag.remove_total_tracks(),
//                                _ => tag.set_total_tracks(t as u32),
//                            }
//                        }
//                        if let Some(t) = self.disc_number {
//                            match t {
//                                0 => tag.remove_disc(),
//                                _ => tag.set_disc(t as u32),
//                            }
//                        }
//                        if let Some(t) = self.total_discs {
//                            match t {
//                                0 => tag.remove_total_discs(),
//                                _ => tag.set_total_discs(t as u32),
//                            }
//                        }
//
//                        tag
//                    }
//                    Err(_) => id3::Tag::default(),
//                };
//
//                tag.write_to_path(path, id3::Version::Id3v24)?;
//            }
//            "m4a" | "m4b" | "m4p" | "m4v" => {
//                let tag = match mp4ameta::Tag::read_from_path(path) {
//                    Ok(mut tag) => {
//                        if let Some(a) = &self.artist {
//                            match a.is_empty() {
//                                true => tag.remove_artists(),
//                                false => tag.set_artist(a),
//                            }
//                        }
//                        if let Some(a) = &self.album_artist {
//                            match a.is_empty() {
//                                true => tag.remove_album_artists(),
//                                false => tag.set_album_artist(a),
//                            }
//                        }
//                        if let Some(a) = &self.album {
//                            match a.is_empty() {
//                                true => tag.remove_album(),
//                                false => tag.set_album(a),
//                            }
//                        }
//                        if let Some(t) = &self.title {
//                            match t.is_empty() {
//                                true => tag.remove_title(),
//                                false => tag.set_title(t),
//                            }
//                        }
//                        if let Some(t) = self.track_number {
//                            match t {
//                                0 => tag.remove_track_number(),
//                                _ => tag.set_track_number(t),
//                            }
//                        }
//                        if let Some(t) = self.total_tracks {
//                            match t {
//                                0 => tag.remove_total_tracks(),
//                                _ => tag.set_total_tracks(t),
//                            }
//                        }
//                        if let Some(t) = self.disc_number {
//                            match t {
//                                0 => tag.remove_disc_number(),
//                                _ => tag.set_disc_number(t),
//                            }
//                        }
//                        if let Some(t) = self.total_discs {
//                            match t {
//                                0 => tag.remove_total_discs(),
//                                _ => tag.set_total_discs(t),
//                            }
//                        }
//
//                        tag
//                    }
//                    Err(_) => mp4ameta::Tag::default(),
//                };
//
//                tag.write_to_path(path)?;
//            }
//            _ => (),
//        }
//
//        Ok(())
//    }
//}
