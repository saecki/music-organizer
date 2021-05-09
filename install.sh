#!/bin/sh

cargo build --release
sudo cp target/release/cli /usr/local/bin/music-organizer

case "$SHELL" in
    *bash)
	echo "creating a completion script for bash"
	/usr/local/bin/music-organizer -g "bash" | sudo tee /etc/bash_completion.d/music-organizer > /dev/null
	;;
    *zsh)
	echo "creating a completion script for zsh"
	/usr/local/bin/music-organizer -g "zsh" | sudo tee /usr/share/zsh/site-functions/_music-organizer > /dev/null
	;;
    *)
	echo "create a completion script for your shell manually by running 'music-organizer --generate-completion <shell>'"
	;;
esac

