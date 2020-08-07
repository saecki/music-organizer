use std::path::PathBuf;
use std::process::exit;
use walkdir::WalkDir;
use std::ffi::{OsStr, OsString};
use clap::{App, Arg, Shell};
use std::str::FromStr;
use std::io::Write;

const MUSIC_FILE_EXTENSIONS: [&str; 5] = [
    "m4a",
    "mp3",
    "m4b",
    "m4p",
    "m4v",
];

#[derive(Debug, PartialEq)]
pub struct Artist {
    pub name: String,
    pub albums: Vec<Album>,
}

#[derive(Debug, PartialEq)]
pub struct Album {
    pub name: String,
    pub songs: Vec<usize>,
}

#[derive(Default, Debug, PartialEq)]
pub struct Song {
    pub track: u16,
    pub title: String,
    pub current_file: PathBuf,
}

#[derive(Default, Debug, PartialEq)]
pub struct Metadata {
    pub track: u16,
    pub artist: String,
    pub album: String,
    pub title: String,
}

impl Metadata {
    pub fn read_from(path: &PathBuf) -> Self {
        if let Ok(tag) = id3::Tag::read_from_path(&path) {
            let track = match tag.track() {
                Some(t) => t as u16,
                None => 0,
            };

            Self {
                track,
                title: tag.title().unwrap_or("").to_string(),
                artist: tag.artist().unwrap_or("").to_string(),
                album: tag.album().unwrap_or("").to_string(),
            }
        } else if let Ok(tag) = mp4ameta::Tag::read_from_path(&path) {
            let track = match tag.track_number() {
                Some((t, _)) => t as u16,
                None => 0,
            };

            Self {
                track,
                title: tag.title().unwrap_or("").to_string(),
                artist: tag.artist().unwrap_or("").to_string(),
                album: tag.album().unwrap_or("").to_string(),
            }
        } else {
            Self::default()
        }
    }
}

fn main() {
    let app = App::new("playlist localizer")
        .version("0.2.0")
        .author("Saecki")
        .about("Finds the local paths to your playlists' songs.")
        .arg(Arg::with_name("music-dir")
            .short("m")
            .long("music-dir")
            .help("The directory which will be searched for playlists and music files")
            .takes_value(true)
            .required_unless("generate-completion")
            .conflicts_with("generate-completion"))
        .arg(Arg::with_name("output-dir")
            .short("o")
            .long("output-dir")
            .help("The directory which the playlists will be written to")
            .takes_value(true)
            .required(true))
        .arg(Arg::with_name("copy")
            .short("c")
            .long("copy")
            .help("Copy the files instead of moving")
            .takes_value(false))
        .arg(Arg::with_name("assume-yes")
            .short("y")
            .long("assume-yes")
            .help("Assumes yes as a answer for all questions")
            .takes_value(false))
        .arg(Arg::with_name("generate-completion")
            .short("g")
            .long("generate-completion")
            .value_name("shell")
            .help("Generates a completion script for the specified shell")
            .conflicts_with("music-dir")
            .takes_value(true)
            .possible_values(&Shell::variants())
        );

    let matches = app.clone().get_matches();
    let output_dir = PathBuf::from(matches.value_of("output-dir").unwrap());
    let generate_completion = matches.value_of("generate-completion").unwrap_or("");

    let abs_output_dir = match PathBuf::from(&output_dir).canonicalize() {
        Ok(t) => t,
        Err(e) => {
            println!("not a valid output dir path: {}\n{:?}", output_dir.display(), e);
            exit(1)
        }
    };

    if generate_completion != "" {
        println!("generating completions...");
        let shell = Shell::from_str(generate_completion).unwrap();
        app.clone().gen_completions("playlist_localizer", shell, abs_output_dir);
        println!("done");
        exit(0);
    }

    let music_dir = PathBuf::from(matches.value_of("music-dir").unwrap());
    let copy = matches.is_present("copy");
    let yes = matches.is_present("copy");

    let abs_music_dir = match PathBuf::from(&music_dir).canonicalize() {
        Ok(t) => t,
        Err(e) => {
            println!("Not a valid music dir path: {}\n{:?}", music_dir.display(), e);
            exit(1)
        }
    };

    println!("indexing...");
    let mut artists = Vec::new();
    let mut unknown = Vec::new();
    let mut songs = Vec::new();

    'songs: for d in WalkDir::new(&abs_music_dir).into_iter()
        .filter_entry(|e| !e.file_name()
            .to_str()
            .map(|s| s.starts_with('.'))
            .unwrap_or(false)
        )
        .filter_map(|e| e.ok())
        .filter(|e| match e.metadata() {
            Ok(m) => m.is_file(),
            Err(_e) => false,
        })
    {
        let p = d.into_path();
        if !is_music_extension(p.extension().unwrap()) { continue; }

        let m = Metadata::read_from(&p);
        let song_index = songs.len();
        songs.push(Song {
            track: m.track,
            title: m.title,
            current_file: p,
        });

        print!("\rsong {}           ", song_index);
        let _ = std::io::stdout().flush().is_ok();

        if m.artist.is_empty() {
            unknown.push(song_index);
            continue;
        }

        if artists.is_empty() {
            artists.push(Artist {
                name: m.artist,
                albums: vec![Album {
                    name: m.album,
                    songs: vec![song_index],
                }],
            });

            continue;
        }

        for ar in &mut artists {
            if ar.name == m.artist {
                for al in &mut ar.albums {
                    if al.name == m.album {
                        al.songs.push(song_index);
                        continue 'songs;
                    }
                }

                ar.albums.push(Album {
                    name: m.album,
                    songs: vec![song_index],
                });
                continue 'songs;
            }
        }

        artists.push(Artist {
            name: m.artist,
            albums: vec![Album {
                name: m.album,
                songs: vec![song_index],
            }],
        });
    }
    println!();

    if !yes {
        loop {
            println!(
                "{} files will be {}. Continue [y/N]?",
                songs.len(),
                if copy { "copied" } else { "moved" }
            );

            let mut input = String::with_capacity(2);
            if let Err(e) = std::io::stdin().read_line(&mut input) {
                println!("error: {}", e);
            } else if input.to_lowercase() == "y\n" {
                break;
            } else if input.to_lowercase() == "n\n" {
                println!("exiting...");
                exit(1);
            }
        }
    }

    println!("\nwriting...");
    let mut counter: usize = 0;
    for ar in &artists {
        let ar_osstr = OsString::from(&ar.name);
        let ar_dir = output_dir.clone().join(&ar.name);
        if !ar_dir.exists() {
            if let Err(e) = std::fs::create_dir(&ar_dir) {
                println!("error creating dir: {}:\n{}", ar_dir.display(), e);
            }
        }

        for al in &ar.albums {
            let al_osstr = OsString::from(&al.name);
            let al_dir = ar_dir.clone().join(&al_osstr);
            if !al_dir.exists() {
                if let Err(e) = std::fs::create_dir(&al_dir) {
                    println!("error creating dir: {}:\n{}", al_dir.display(), e);
                }
            }

            for si in &al.songs {
                let song = &songs[*si];
                let extension = song.current_file.extension().unwrap();

                if al.name.is_empty() {
                    let mut file_name = OsString::with_capacity(4 + ar_osstr.len() + song.title.len() + extension.len());

                    file_name.push(&ar_osstr);
                    file_name.push(" - ");
                    file_name.push(&song.title);
                    file_name.push(".");
                    file_name.push(extension);

                    let new_file = ar_dir.join(file_name);

                    mv_or_cp(&counter, &song.current_file, &new_file, copy);
                } else {
                    let mut file_name = OsString::with_capacity(9 + ar_osstr.len() + song.title.len() + extension.len());

                    file_name.push(format!("{:02} - ", song.track));
                    file_name.push(&ar_osstr);
                    file_name.push(" - ");
                    file_name.push(&song.title);
                    file_name.push(".");
                    file_name.push(extension);

                    let new_file = al_dir.join(file_name);

                    mv_or_cp(&counter, &song.current_file, &new_file, copy);
                }
                counter += 1;
            }
        }
    }

    if !unknown.is_empty() {
        let unknown_dir = output_dir.join("unknown");
        if !unknown_dir.exists() {
            if let Err(e) = std::fs::create_dir(&unknown_dir) {
                println!("Error creating dir: {}:\n{}", unknown_dir.display(), e);
            }
        }
        for si in &unknown {
            let song = &songs[*si];
            let new_file = unknown_dir.join(song.current_file.file_name().unwrap());

            mv_or_cp(&counter, &song.current_file, &new_file, copy);
            counter += 1;
        }
        println!();
    }

    println!("\ndone")
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

fn mv_or_cp(song_index: &usize, old: &PathBuf, new: &PathBuf, copy: bool) {
    if copy {
        print!("\rcopying {} {}           ", song_index, new.display());
        let _ = std::io::stdout().flush().is_ok();
        if let Err(e) = std::fs::copy(old, new) {
            println!("\nerror{}", e);
        }
    } else {
        print!("\rmoving {} {}           ", song_index, new.display());
        let _ = std::io::stdout().flush().is_ok();
        if let Err(e) = std::fs::rename(old, new) {
            println!("\nerror{}", e);
        }
    }
}
