#!/bin/sh

cargo install --path .

case "$SHELL" in
    *zsh)
	echo "creating a completion script for zsh"
	~/.cargo/bin/music-organizer -g "zsh" > ~/.config/zsh/functions/_music-organizer
	;;
    *)
	echo "create a completion script for your shell manually by running 'music-organizer --generate-completion <shell>'"
	;;
esac

