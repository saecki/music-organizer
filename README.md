# music organizer

## Usage
```
Usage: music-organizer <COMMAND>

Commands:
  organize     Moves/copies, renames and retags Music files using their metadata
  transcode    Generates a completion script for the specified shell
  completions  Generates a completion script for the specified shell
  help         Print this message or the help of the given subcommand(s)

Options:
  -h, --help     Print help
  -V, --version  Print version
```

### Organize command
```
Moves/copies, renames and retags Music files using their metadata

Usage: music-organizer organize [OPTIONS]

Options:
  -m, --music-dir <MUSIC_DIR>                  The directory which will be searched for music files [default: ~/Music]
  -o, --output-dir <OUTPUT_DIR>                The directory which the content will be written to
  -v, --verbosity <level>                      Verbosity level of the output. 0 means least 2 means most verbose ouput
                                               [default: 1]
  -c, --copy                                   Copy the files instead of moving
  -y, --assume-yes                             Assumes yes as a answer for questions
  -d, --dry-run                                Only check files and print actions don't change anything
  -n, --nocheck                                Don't check for inconsistencies
  -e, --embedded-artworks <EMBEDDED_ARTWORKS>  What to do with embedded artworks [default: remove-redundant] [possible
                                               values: keep, remove-redundant, remove]
      --nocleanup                              Don't remove empty directories
  -t, --timings                                Prints timing information
  -h, --help                                   Print help
```

### Transcode command
```
Generates a completion script for the specified shell

Usage: music-organizer transcode [OPTIONS] --output-dir <OUTPUT_DIR>

Options:
  -m, --music-dir <MUSIC_DIR>    The directory which will be searched for music files [default: ~/Music]
  -o, --output-dir <OUTPUT_DIR>  The directory which the content will be written to
  -v, --verbosity <level>        Verbosity level of the output. 0 means least 2 means most verbose ouput [default: 1]
  -t, --timings                  Prints timing information
  -h, --help                     Print help
```

### Completions command
```
Generates a completion script for the specified shell

Usage: music-organizer completions <SHELL>

Arguments:
  <SHELL>  The shell for which the completions are generated [possible values: bash, elvish, fish, powershell, zsh]

Options:
  -h, --help  Print help
```

## Development

__System requirements__

- `libopusenc` development headers for transcoding
