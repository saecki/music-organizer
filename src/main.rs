use std::ffi::{OsStr, OsString};
use std::io::Write;
use std::path::PathBuf;
use std::process::exit;
use std::str::FromStr;

use clap::{App, Arg, Shell};
use walkdir::WalkDir;

const MUSIC_FILE_EXTENSIONS: [&str; 5] = [
    "m4a",
    "mp3",
    "m4b",
    "m4p",
    "m4v",
];

static mut LAST_LEN: usize = 0;

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
    pub artist: String,
    pub title: String,
    pub current_file: PathBuf,
}

#[derive(Default, Debug, PartialEq)]
pub struct Metadata {
    pub track: u16,
    pub artist: String,
    pub album_artist: String,
    pub album: String,
    pub title: String,
}

#[derive(Default, Debug, PartialEq)]
pub struct FileMove {
    old: PathBuf,
    new: PathBuf,
}

#[derive(Default, Debug, PartialEq)]
pub struct DirCreation {
    path: PathBuf,
}

impl Metadata {
    pub fn read_from(path: &PathBuf) -> Self {
        match path.extension().unwrap().to_str().unwrap() {
            "mp3" => if let Ok(tag) = id3::Tag::read_from_path(&path) {
                let track = match tag.track() {
                    Some(t) => t as u16,
                    None => 0,
                };

                return Self {
                    track,
                    artist: tag.artist().unwrap_or("").to_string(),
                    album_artist: tag.album_artist().unwrap_or("").to_string(),
                    title: tag.title().unwrap_or("").to_string(),
                    album: tag.album().unwrap_or("").to_string(),
                };
            } else {},
            "m4a" | "m4b" | "m4p" | "m4v" => if let Ok(tag) = mp4ameta::Tag::read_from_path(&path) {
                let track = match tag.track_number() {
                    (Some(t), _) => t as u16,
                    (None, _) => 0,
                };

                return Self {
                    track,
                    artist: tag.artist().unwrap_or("").to_string(),
                    album_artist: tag.album_artist().unwrap_or("").to_string(),
                    title: tag.title().unwrap_or("").to_string(),
                    album: tag.album().unwrap_or("").to_string(),
                };
            },
            _ => (),
        }

        Self::default()
    }
}

fn main() {
    let app = App::new("music organizer")
        .version("0.1.0")
        .author("Saecki")
        .about("Moves or copies and renames Music files using their metadata information.")
        .arg(Arg::with_name("music-dir")
            .short("m")
            .long("music-dir")
            .help("The directory which will be searched for music files")
            .takes_value(true)
            .required_unless("generate-completion")
            .conflicts_with("generate-completion"))
        .arg(Arg::with_name("output-dir")
            .short("o")
            .long("output-dir")
            .help("The directory which the content will be written to")
            .takes_value(true))
        .arg(Arg::with_name("copy")
            .short("c")
            .long("copy")
            .help("Copy the files instead of moving")
            .requires("output-dir")
            .takes_value(false))
        .arg(Arg::with_name("assume-yes")
            .short("y")
            .long("assume-yes")
            .help("Assumes yes as a answer for all questions")
            .takes_value(false))
        .arg(Arg::with_name("verbose")
            .short("v")
            .long("verbose")
            .help("Verbose output")
            .takes_value(false))
        .arg(Arg::with_name("generate-completion")
            .short("g")
            .long("generate-completion")
            .value_name("shell")
            .help("Generates a completion script for the specified shell")
            .conflicts_with("music-dir")
            .requires("output-dir")
            .takes_value(true)
            .possible_values(&["bash", "zsh", "fish", "elvish", "powershell"])
        );

    let matches = app.clone().get_matches();
    let generate_completion = matches.value_of("generate-completion").unwrap_or("");


    if generate_completion != "" {
        let output_dir = PathBuf::from(matches.value_of("output-dir").unwrap());
        if !output_dir.exists() {
            match std::fs::create_dir_all(&output_dir) {
                Ok(_) => println!("created dir: {}", output_dir.display()),
                Err(e) => println!("error creating dir: {}\n{}", output_dir.display(), e),
            }
        }

        println!("generating completions...");
        let shell = Shell::from_str(generate_completion).unwrap();
        app.clone().gen_completions("music_organizer", shell, output_dir);
        println!("done");
        exit(0);
    }

    let music_dir = PathBuf::from(matches.value_of("music-dir").unwrap());
    let copy = matches.is_present("copy");
    let yes = matches.is_present("assume-yes");
    let verbose = matches.is_present("verbose");

    let abs_music_dir = match PathBuf::from(&music_dir).canonicalize() {
        Ok(t) => t,
        Err(e) => {
            println!("Not a valid music dir path: {}\n{:?}", music_dir.display(), e);
            exit(1)
        }
    };

    let output_dir = match matches.value_of("output-dir") {
        Some(s) => PathBuf::from(s),
        None => abs_music_dir.clone(),
    };

    if !output_dir.exists() {
        match std::fs::create_dir_all(&output_dir) {
            Ok(_) => println!("created dir: {}", output_dir.display()),
            Err(e) => println!("error creating dir: {}\n{}", output_dir.display(), e),
        }
    }


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
        .filter(|e| e.metadata().map(|m| m.is_file()).unwrap_or(false))
    {
        let p = d.into_path();
        if !is_music_extension(p.extension().unwrap()) { continue; }

        let m = Metadata::read_from(&p);
        let song_index = songs.len();

        print_verbose(&format!("{} {} - {}", song_index + 1, &m.artist, &m.title), verbose);

        songs.push(Song {
            track: m.track,
            artist: m.artist.clone(),
            title: m.title,
            current_file: p,
        });

        let _ = std::io::stdout().flush().is_ok();

        let artist = if !m.album_artist.is_empty() {
            m.album_artist
        } else if !m.artist.is_empty() {
            m.artist
        } else {
            unknown.push(song_index);
            continue;
        };

        if artists.is_empty() {
            artists.push(Artist {
                name: artist,
                albums: vec![Album {
                    name: m.album,
                    songs: vec![song_index],
                }],
            });

            continue;
        }

        for ar in &mut artists {
            if ar.name == artist {
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
            name: artist,
            albums: vec![Album {
                name: m.album,
                songs: vec![song_index],
            }],
        });
    }

    println!("\nchecking...");

    let mut offset = 1;
    for ar1 in artists.iter() {
        for ar2 in artists.iter().skip(offset) {
            if ar1.name.eq_ignore_ascii_case(&ar2.name) {
                println!("These two artists are named similarly:\n{}\n{}", &ar1.name, &ar2.name);
                let index = input_options_loop(&[
                    "don't do anything",
                    "merge using first",
                    "merge using second",
                    "enter new name",
                ]);

                match index {
                    0 => continue,
                    1 => println!("merging using first"),
                    2 => println!("merging using second"),
                    3 => loop {
                        let new_name = input_loop("enter new name:", |_| true);
                        println!("new name: '{}'", new_name);

                        let index = input_options_loop(&[
                            "ok",
                            "reenter name",
                            "dismiss",
                        ]);

                        match index {
                            0 => {
                                //TODO: rename
                                break;
                            }
                            1 => continue,
                            _ => break,
                        }
                    }
                    _ => continue,
                }
            }
        }
        offset += 1;
    }

    let mut dir_creations = Vec::new();
    let mut file_moves = Vec::with_capacity(songs.len() / 10);

    for ar in &artists {
        let ar_dir = output_dir.join(valid_os_string(&ar.name));
        if !ar_dir.exists() {
            dir_creations.push(DirCreation { path: ar_dir.clone() });
        }

        for al in &ar.albums {
            let single_album_name = format!("{} - single", songs[al.songs[0]].title.to_lowercase());
            let is_single = al.name.is_empty() || al.songs.len() == 1 && al.name.to_lowercase() == single_album_name;
            let al_dir = ar_dir.join(valid_os_string(&al.name));

            if !is_single && !al_dir.exists() {
                dir_creations.push(DirCreation { path: al_dir.clone() });
            }

            for si in &al.songs {
                let song = &songs[*si];
                let extension = song.current_file.extension().unwrap();

                let new_file;
                if is_single {
                    let mut file_name = OsString::with_capacity(4 + song.artist.len() + song.title.len() + extension.len());

                    file_name.push(valid_os_string(&song.artist));
                    file_name.push(" - ");
                    file_name.push(valid_os_string(&song.title));
                    file_name.push(".");
                    file_name.push(extension);

                    new_file = ar_dir.join(file_name);
                } else {
                    let mut file_name = OsString::with_capacity(9 + song.artist.len() + song.title.len() + extension.len());

                    file_name.push(format!("{:02} - ", song.track));
                    file_name.push(valid_os_string(&song.artist));
                    file_name.push(" - ");
                    file_name.push(valid_os_string(&song.title));
                    file_name.push(".");
                    file_name.push(extension);

                    new_file = al_dir.join(file_name);
                }

                if new_file != song.current_file {
                    file_moves.push(FileMove {
                        old: song.current_file.clone(),
                        new: new_file,
                    });
                }
            }
        }
    }

    if !unknown.is_empty() {
        let unknown_dir = output_dir.join("unknown");
        if !unknown_dir.exists() {
            dir_creations.push(DirCreation { path: unknown_dir.clone() });
        }
        for si in &unknown {
            let song = &songs[*si];
            let new_file = unknown_dir.join(song.current_file.file_name().unwrap());

            file_moves.push(FileMove {
                old: song.current_file.clone(),
                new: new_file,
            });
        }
    }
    println!();

    if dir_creations.is_empty() && file_moves.is_empty() {
        println!("noting to do exiting...");
        exit(0);
    }

    if !yes {
        if verbose {
            if !dir_creations.is_empty() {
                println!("dirs:");
                for (i, d) in dir_creations.iter().enumerate() {
                    println!("{} {}", i + 1, d.path.display());
                }
                println!();
            }
            if !file_moves.is_empty() {
                println!("files:");
                for (i, f) in file_moves.iter().enumerate() {
                    println!("{} {}", i + 1, f.new.display());
                }
                println!();
            }
        }

        let ok = input_confirmation_loop(&format!(
            "{} dirs will be created.\n{} files will be {}. Continue",
            dir_creations.len(),
            file_moves.len(),
            if copy { "copied" } else { "moved" })
        );

        if !ok {
            println!("exiting...");
            exit(1);
        }
    }

    println!("\nwriting...");

    unsafe {
        LAST_LEN = 0;
    }
    for (i, d) in dir_creations.iter().enumerate() {
        match std::fs::create_dir(&d.path) {
            Ok(_) => print_verbose(&format!("{} creating dir {}", i, d.path.display()), verbose),
            Err(e) => println!("error creating dir: {}:\n{}", d.path.display(), e),
        }
    }
    println!();

    unsafe {
        LAST_LEN = 0;
    }
    for (i, f) in file_moves.iter().enumerate() {
        mv_or_cp(&(i + 1), &f.old, &f.new, copy, verbose);
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

fn mv_or_cp(song_index: &usize, old: &PathBuf, new: &PathBuf, copy: bool, verbose: bool) {
    if copy {
        print_verbose(&format!("{} copying {}", song_index, new.display()), verbose);
        let _ = std::io::stdout().flush().is_ok();
        if let Err(e) = std::fs::copy(old, new) {
            println!("\nerror: {}", e);
        }
    } else {
        print_verbose(&format!("{} moving {}", song_index, new.display()), verbose);
        let _ = std::io::stdout().flush().is_ok();
        if let Err(e) = std::fs::rename(old, new) {
            println!("\nerror: {}", e);
        }
    }
}

fn input_loop(str: &str, predicate: fn(&str) -> bool) -> String {
    let mut input = String::with_capacity(10);

    loop {
        println!("{}", str);

        match std::io::stdin().read_line(&mut input) {
            Ok(_) => if predicate(&input) { return input; },
            Err(e) => println!("error:\n {}", e),
        }
    }
}

fn input_options_loop(options: &[&str]) -> usize {
    let mut input = String::with_capacity(2);

    loop {
        for (i, s) in options.iter().enumerate() {
            println!("[{}] {}", i, s);
        }

        match std::io::stdin().read_line(&mut input) {
            Ok(_) => match usize::from_str(input.trim_matches('\n')) {
                Ok(i) => if i < options.len() {
                    return i;
                } else {
                    println!("invalid input")
                },
                Err(_) => println!("invalid input"),
            }
            Err(e) => println!("error:\n {}", e),
        }
    }
}

fn input_confirmation_loop(str: &str) -> bool {
    let mut input = String::with_capacity(2);

    loop {
        print!("{} [y/N]?", str);
        let _ = std::io::stdout().flush().is_ok();

        if let Err(e) = std::io::stdin().read_line(&mut input) {
            println!("error:\n {}", e);
        } else {
            input.make_ascii_lowercase();

            if input == "\n" || input == "y\n" {
                return true;
            } else if input == "n\n" {
                return false;
            } else {
                println!("invalid input");
            }
        }
    }
}

#[inline]
fn print_verbose(str: &str, verbose: bool) {
    if verbose {
        println!("{}", str);
    } else {
        let len = str.chars().count();
        let diff = unsafe { LAST_LEN as i32 - len as i32 };

        print!("\r{}", str);
        for _ in 0..diff {
            print!(" ");
        }
        let _ = std::io::stdout().flush().is_ok();

        unsafe {
            LAST_LEN = len;
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
