#!/bin/sh

cargo build --release
sudo cp target/release/music_organizer /usr/local/bin

case "$SHELL" in
    *zsh)
	echo "creating a completion script for zsh"
	sudo /usr/local/bin/music_organizer -g "zsh" -o /usr/share/zsh/site-functions/
	;;
    *bash)
	echo "creating a completion script for bash"
	sudo /usr/local/bin/music_organizer -g "bash" -o /etc/bash_completion.d/
	;;
    *)
	echo "create a completion script for your shell manually by running 'music_organizer --generate-completion <shell>'"
	;;
esac

