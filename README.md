# music organizer

### Usage
```
music organizer 0.1.0
Saecki <tobiasschmitz2001@gmail.com>
Moves/copies, renames and retags Music files using their metadata.

USAGE:
    music_organizer [FLAGS] [OPTIONS]

FLAGS:
    -y, --assume-yes    Assumes yes as a answer for questions
    -c, --copy          Copy the files instead of moving
    -d, --dryrun        Only check files don't change anything
    -h, --help          Prints help information
    -n, --nocheck       Don't check for inconsistencies
    -V, --version       Prints version information

OPTIONS:
    -g, --generate-completion <shell>
            Generates a completion script for the specified shell [possible values: bash, zsh, fish,
            elvish, powershell]

    -m, --music-dir <music-dir>          The directory which will be searched for music files
    -o, --output-dir <output-dir>        The directory which the content will be written to
    -v, --verbosity <level>
            Verbosity level of the output. 0 means least 2 means most verbose ouput. [default: 1]
            [possible values: 0, 1, 2]
```
