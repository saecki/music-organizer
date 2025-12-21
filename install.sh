#!/bin/sh

cargo install --profile=release-lto --path .

case "$SHELL" in
    *zsh)
	echo "creating a completion script for zsh"
	~/.cargo/bin/music-organizer completions "zsh" > ~/.config/zsh/functions/_music-organizer
	;;
    *)
	echo "create a completion script for your shell manually by running 'music-organizer --generate-completion <shell>'"
	;;
esac

