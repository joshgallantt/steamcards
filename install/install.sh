#!/bin/sh
# Installs or updates steamcards on macOS and Linux, from its GitHub releases.
#
#   curl -fsSL https://raw.githubusercontent.com/joshgallantt/steamcards/main/install/install.sh | sh
#
# Run it again to update: it compares your version with the latest release,
# and only downloads when the release is newer. All optional:
#
#   STEAMCARDS_VERSION       the release to install, like 0.2.0 (default: the latest)
#   STEAMCARDS_FORCE=1       reinstall, even if that version is already installed
#   STEAMCARDS_INSTALL_DIR   the folder to install to (default: ~/.local/bin, or
#                             wherever steamcards already is)
#   STEAMCARDS_RELEASES_URL  a mirror to download from (default:
#                             https://github.com/joshgallantt/steamcards/releases),
#                             laid out the same way: <url>/latest redirects to
#                             <url>/tag/vX.Y.Z, and the files are in <url>/download/vX.Y.Z/
#
# It never uses sudo, so the folder has to be one you can write to.

set -eu

repo=joshgallantt/steamcards

say() {
    printf '%s\n' "$*"
}

fail() {
    printf 'steamcards install: %s\n' "$*" >&2
    exit 1
}

# Prints -1, 0 or 1 as version $1 is older than, the same as, or newer than
# version $2, by semver's rules: numbers compare as numbers, and a
# pre-release (1.0.0-rc.1) comes before its release.
compare_versions() {
    awk -v a="$1" -v b="$2" '
        function cmp(x, y,    xs, ys, n, m, i, xnum, ynum) {
            n = split(x, xs, ".")
            m = split(y, ys, ".")
            for (i = 1; i <= n && i <= m; i++) {
                xnum = (xs[i] ~ /^[0-9]+$/)
                ynum = (ys[i] ~ /^[0-9]+$/)
                if (xnum && ynum) {
                    if (xs[i] + 0 < ys[i] + 0) return -1
                    if (xs[i] + 0 > ys[i] + 0) return 1
                } else if (xnum != ynum) {
                    return (xnum ? -1 : 1)
                } else if (xs[i] != ys[i]) {
                    return ((xs[i] < ys[i]) ? -1 : 1)
                }
            }
            if (n < m) return -1
            if (n > m) return 1
            return 0
        }
        BEGIN {
            sub(/\+.*/, "", a)
            sub(/\+.*/, "", b)
            ai = index(a, "-")
            bi = index(b, "-")
            acore = ai ? substr(a, 1, ai - 1) : a
            bcore = bi ? substr(b, 1, bi - 1) : b
            apre = ai ? substr(a, ai + 1) : ""
            bpre = bi ? substr(b, bi + 1) : ""
            c = cmp(acore, bcore)
            if (c == 0 && apre != bpre) {
                if (apre == "") c = 1
                else if (bpre == "") c = -1
                else c = cmp(apre, bpre)
            }
            print c
        }'
}

command -v curl >/dev/null 2>&1 || fail "curl is needed to download steamcards"

# Where the releases are: GitHub, or a mirror laid out the same way. curl
# sticks to https, unless a mirror's address is http.
releases=${STEAMCARDS_RELEASES_URL:-https://github.com/$repo/releases}
releases=${releases%/}
case "$releases" in
    http://*) proto='=http,https' ;;
    *) proto='=https' ;;
esac

# Which build to download.
case "$(uname -s)" in
    Darwin) os=apple-darwin ;;
    Linux) os=unknown-linux-musl ;;
    *) fail "there's no prebuilt steamcards for $(uname -s). See https://github.com/$repo/blob/main/CONTRIBUTING.md#building-from-source" ;;
esac
case "$(uname -m)" in
    x86_64 | amd64) arch=x86_64 ;;
    arm64 | aarch64) arch=aarch64 ;;
    *) fail "there's no prebuilt steamcards for $(uname -m) processors. See https://github.com/$repo/blob/main/CONTRIBUTING.md#building-from-source" ;;
esac
# A shell running under Rosetta says x86_64 on an Apple silicon Mac.
if [ "$os" = apple-darwin ] && [ "$arch" = x86_64 ] &&
    [ "$(sysctl -n sysctl.proc_translated 2>/dev/null || true)" = 1 ]; then
    arch=aarch64
fi
asset="steamcards-$arch-$os.tar.gz"

# Which version.
if [ -n "${STEAMCARDS_VERSION:-}" ]; then
    version=${STEAMCARDS_VERSION#v}
else
    # The latest release's page redirects to its tag: .../releases/tag/v0.2.0.
    latest=$(curl --proto "$proto" --tlsv1.2 -fsSLI -o /dev/null -w '%{url_effective}' "$releases/latest") ||
        fail "couldn't find a release at $releases"
    case "$latest" in
        */tag/v*) version=${latest##*/tag/v} ;;
        *) fail "couldn't find a release at $releases" ;;
    esac
fi
case "$version" in
    "" | *[!0-9A-Za-z.+-]*) fail "\"$version\" isn't a version steamcards has" ;;
esac

# Which steamcards is installed already, if any: the one in the install
# folder or, unless a folder was chosen, the first one on PATH.
dir=${STEAMCARDS_INSTALL_DIR:-$HOME/.local/bin}
current=""
if [ -x "$dir/steamcards" ]; then
    current=$dir/steamcards
elif [ -z "${STEAMCARDS_INSTALL_DIR:-}" ]; then
    current=$(command -v steamcards 2>/dev/null || true)
    case "$current" in
        /*) dir=$(dirname "$current") ;;
        *) current="" ;;
    esac
fi

# Its version, from "steamcards 0.1.0". Builds from before --version
# existed print nothing here, and are replaced.
installed=""
order=""
if [ -n "$current" ]; then
    installed=$("$current" --version </dev/null 2>/dev/null |
        sed -n 's/^steamcards \([0-9][0-9A-Za-z.+-]*\).*/\1/p' || true)
fi
if [ -n "$installed" ]; then
    order=$(compare_versions "$installed" "$version")
    if [ "${STEAMCARDS_FORCE:-}" != 1 ]; then
        if [ "$order" = 0 ]; then
            say "steamcards $installed is already up to date."
            exit 0
        fi
        if [ "$order" = 1 ] && [ -z "${STEAMCARDS_VERSION:-}" ]; then
            say "steamcards $installed is newer than the latest release, $version. Leaving it as it is."
            exit 0
        fi
    fi
fi

# A copy that Homebrew or cargo installed is theirs to update.
if [ -n "$current" ]; then
    case "$current" in
        "${CARGO_HOME:-$HOME/.cargo}/bin/steamcards")
            fail "$current was built from source with cargo. Update it the same way (https://github.com/$repo/blob/main/CONTRIBUTING.md#building-from-source), or set STEAMCARDS_INSTALL_DIR to install a prebuilt copy somewhere else." ;;
    esac
    case "$(readlink "$current" 2>/dev/null || true)" in
        *Cellar/*)
            fail "$current came from Homebrew. Update it with: brew upgrade steamcards" ;;
    esac
fi

mkdir -p "$dir" 2>/dev/null ||
    fail "couldn't create $dir. Set STEAMCARDS_INSTALL_DIR to a folder you own."
[ -w "$dir" ] ||
    fail "can't write to $dir, and this script doesn't use sudo. Set STEAMCARDS_INSTALL_DIR to a folder you own."

tmp=$(mktemp -d) || fail "couldn't create a temporary folder"
new="$dir/.steamcards.new.$$"
trap 'rm -rf "$tmp"; rm -f "$new"' EXIT
trap 'exit 1' HUP INT TERM

base="$releases/download/v$version"
say "Downloading steamcards $version for $arch-$os..."
curl --proto "$proto" --tlsv1.2 -fsSL -o "$tmp/$asset" "$base/$asset" ||
    fail "couldn't download $base/$asset"
curl --proto "$proto" --tlsv1.2 -fsSL -o "$tmp/SHA256SUMS" "$base/SHA256SUMS" ||
    fail "couldn't download $base/SHA256SUMS"

expected=$(awk -v f="$asset" '$2 == f || $2 == "*" f { print $1 }' "$tmp/SHA256SUMS")
[ -n "$expected" ] || fail "SHA256SUMS doesn't list $asset"
if command -v sha256sum >/dev/null 2>&1; then
    actual=$(sha256sum "$tmp/$asset" | cut -d' ' -f1)
elif command -v shasum >/dev/null 2>&1; then
    actual=$(shasum -a 256 "$tmp/$asset" | cut -d' ' -f1)
else
    fail "sha256sum or shasum is needed to check the download"
fi
[ "$actual" = "$expected" ] ||
    fail "$asset doesn't match its checksum in SHA256SUMS, so nothing was installed"

tar -xzf "$tmp/$asset" -C "$tmp" steamcards || fail "couldn't unpack $asset"

# Copied in beside the old one under a temporary name, then renamed over it.
# The rename is atomic, and a steamcards that's running keeps its old copy.
cp "$tmp/steamcards" "$new" || fail "couldn't write to $dir"
chmod 755 "$new"
mv -f "$new" "$dir/steamcards" || fail "couldn't replace $dir/steamcards"

if [ -z "$current" ]; then
    say "Installed steamcards $version to $dir/steamcards."
elif [ -z "$installed" ]; then
    say "Updated steamcards to $version."
elif [ "$order" = -1 ]; then
    say "Updated steamcards $installed → $version."
elif [ "$order" = 1 ]; then
    say "Downgraded steamcards $installed → $version."
else
    say "Reinstalled steamcards $version."
fi

case ":$PATH:" in
    *":$dir:"*)
        [ -n "$current" ] || say "Run it with: steamcards"
        ;;
    *)
        say ""
        say "$dir isn't on your PATH yet. Add this line to your shell's startup file"
        say "(~/.zshrc, ~/.bashrc or similar), then open a new terminal:"
        say ""
        say "    export PATH=\"$dir:\$PATH\""
        ;;
esac
