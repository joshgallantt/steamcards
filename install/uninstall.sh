#!/bin/sh
# Uninstalls steamcards from macOS and Linux.
#
#   curl -fsSL https://raw.githubusercontent.com/joshgallantt/steamcards/main/install/uninstall.sh | sh
#
# It finds steamcards as install.sh does, in ~/.local/bin or else the first
# one on your PATH, and removes it the way it was installed: a copy from
# Homebrew with brew, one that cargo built with cargo, and any other by
# deleting it. Then it asks whether to delete your sign-in and choices too.
# With no terminal to ask in, it keeps them. To answer before it asks:
#
#   STEAMCARDS_DELETE_DATA=1  delete your sign-in and choices, without asking
#   STEAMCARDS_DELETE_DATA=0  keep them, without asking
#
# like this:
#
#   curl -fsSL https://raw.githubusercontent.com/joshgallantt/steamcards/main/install/uninstall.sh | STEAMCARDS_DELETE_DATA=1 sh
#
# It never uses sudo.

set -eu

tap=joshgallantt/steamcards

say() {
    printf '%s\n' "$*"
}

fail() {
    printf 'steamcards uninstall: %s\n' "$*" >&2
    exit 1
}

# Everything it looks for is in your home folder.
case "${HOME:-}" in
    /*) ;;
    *) fail "HOME isn't set, so there's no telling where steamcards is" ;;
esac

# Your sign-in, choices and saved prices are in the folder steamcards keeps
# them in: the one Rust's dirs crate gives it for settings
# (dirs::config_dir), plus steamcards. It's the only folder this ever
# deletes.
if [ "$(uname -s)" = Darwin ]; then
    data="$HOME/Library/Application Support/steamcards"
else
    # dirs goes by XDG_CONFIG_HOME when that's a full path.
    case "${XDG_CONFIG_HOME:-}" in
        /*) data="$XDG_CONFIG_HOME/steamcards" ;;
        *) data="$HOME/.config/steamcards" ;;
    esac
fi

# Which steamcards is installed, if any: the one in install.sh's folder, or
# else the first one on PATH.
current="$HOME/.local/bin/steamcards"
if [ ! -x "$current" ]; then
    current=$(command -v steamcards 2>/dev/null || true)
    case "$current" in
        /*) ;;
        *) current="" ;;
    esac
fi

# How it was installed: Homebrew links it from its Cellar, and cargo keeps
# what it builds in a bin folder of its own.
how=""
if [ -n "$current" ]; then
    how=copy
    case "$current" in
        "${CARGO_HOME:-$HOME/.cargo}/bin/steamcards") how=cargo ;;
    esac
    case "$(readlink "$current" 2>/dev/null || true)" in
        *Cellar/*) how=homebrew ;;
    esac
fi

case "$how" in
    homebrew)
        # The brew beside it is the Homebrew that installed it. A Mac can
        # have two: one in /opt/homebrew, and an older one in /usr/local.
        brew=$(dirname "$current")/brew
        if [ ! -x "$brew" ]; then
            brew=$(command -v brew 2>/dev/null || true)
        fi
        [ -n "$brew" ] ||
            fail "steamcards came from Homebrew, but brew isn't on your PATH. Uninstall it with: brew uninstall steamcards && brew untap $tap"
        say "steamcards came from Homebrew. Uninstalling it with: brew uninstall steamcards"
        "$brew" uninstall steamcards || fail "brew couldn't uninstall steamcards"
        # This repository is its own tap, which nothing needs any more.
        if "$brew" tap | grep -x "$tap" >/dev/null; then
            "$brew" untap "$tap" ||
                say "brew couldn't remove the steamcards tap. Remove it with: brew untap $tap"
        fi
        ;;
    cargo)
        cargo=$(command -v cargo 2>/dev/null || true)
        [ -n "$cargo" ] ||
            fail "steamcards was built from source with cargo, but cargo isn't on your PATH. Uninstall it with: cargo uninstall steamcards"
        say "steamcards was built from source with cargo. Uninstalling it with: cargo uninstall steamcards"
        "$cargo" uninstall steamcards || fail "cargo couldn't uninstall steamcards"
        ;;
    copy)
        folder=$(dirname "$current")
        [ -w "$folder" ] ||
            fail "steamcards is in $folder, which only an administrator can change, and this script doesn't use sudo. Delete it with: sudo rm \"$current\""
        say "Deleting $current."
        rm -f "$current" || fail "couldn't delete $current"
        ;;
esac

# Your sign-in and choices go too, if you say so.
data_left=none
if [ -d "$data" ]; then
    if [ -n "$current" ]; then
        question="Also delete your sign-in and choices?"
    else
        say "steamcards isn't installed."
        question="Delete your sign-in and choices?"
    fi
    answer=""
    case "${STEAMCARDS_DELETE_DATA:-}" in
        1) answer=y ;;
        0) answer=n ;;
        *)
            # What's coming in on stdin is this script, from curl, so the
            # question goes to the terminal, when there is one.
            if (: </dev/tty) 2>/dev/null; then
                printf '%s [y/N] ' "$question" >/dev/tty
                read -r answer </dev/tty || answer=""
            fi
            ;;
    esac
    case "$answer" in
        [Yy] | [Yy][Ee][Ss])
            rm -rf "$data" || fail "couldn't delete $data"
            data_left=deleted
            ;;
        *) data_left=kept ;;
    esac
fi

if [ -n "$current" ]; then
    case "$data_left" in
        deleted) say "Uninstalled steamcards and deleted your sign-in and choices." ;;
        kept) say "Uninstalled steamcards. Your sign-in and choices are still in $data." ;;
        *) say "Uninstalled steamcards." ;;
    esac
else
    case "$data_left" in
        deleted) say "Deleted your sign-in and choices." ;;
        kept) say "Nothing was removed. Your sign-in and choices are still in $data." ;;
        *) say "steamcards isn't installed, so there's nothing to remove." ;;
    esac
fi
