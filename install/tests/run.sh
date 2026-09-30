#!/bin/sh
# Tests install.sh against a local stand-in for steamcards' GitHub releases,
# and uninstall.sh against stand-ins for Homebrew and cargo.
#
#   sh install/tests/run.sh             under sh, dash and busybox sh, where installed
#   sh install/tests/run.sh dash bash   under the shells named
#
# Each scenario pipes install.sh or uninstall.sh into the shell under test, as
# `curl ... | sh` does, with nothing from this environment: just a HOME,
# TMPDIR and PATH of its own in a temporary folder, and the stand-in's address
# in STEAMCARDS_RELEASES_URL. So it never sees or touches a steamcards that's
# really installed, or your sign-in and choices. uninstall.sh runs with no
# terminal, as on CI, or with one of its own that answers its question
# (terminal.py). It prints a line per scenario, and exits with 1 if any
# failed. It needs python3, curl and tar.

set -u

here=$(cd "$(dirname "$0")" && pwd)
installer=$(cd "$here/.." && pwd)/install.sh
uninstaller=$(cd "$here/.." && pwd)/uninstall.sh

if [ "$#" -gt 0 ]; then
    shells=$*
else
    shells=""
    for candidate in sh dash busybox; do
        if command -v "$candidate" >/dev/null 2>&1; then
            shells="${shells:+$shells }$candidate"
        fi
    done
fi

# The scenarios' PATH: the system's own folders, plus whatever a scenario
# adds. Never the PATH this runs with, which may have a real steamcards on it.
system_path=/usr/bin:/bin:/usr/sbin:/sbin
if (PATH=$system_path && command -v steamcards) >/dev/null 2>&1; then
    echo "There's a steamcards in $system_path, which the scenarios would find." >&2
    exit 1
fi
for tool in curl tar awk sed mktemp grep readlink uname; do
    if ! (PATH=$system_path && command -v "$tool") >/dev/null 2>&1; then
        echo "The scripts need $tool, and there's none in $system_path." >&2
        exit 1
    fi
done
python=$(command -v python3) || {
    echo "The tests need python3, for the stand-in for GitHub and terminal.py." >&2
    exit 1
}
system=$(uname -s)

tmp_base=${TMPDIR:-/tmp}
work=$(mktemp -d "${tmp_base%/}/steamcards-install-tests.XXXXXX") || exit 1
server=""
cleanup() {
    if [ -n "$server" ]; then
        kill "$server" 2>/dev/null
    fi
    chmod -R u+w "$work" 2>/dev/null
    rm -rf "$work"
}
trap cleanup EXIT
trap 'exit 130' HUP INT TERM

# macOS's mktemp ignores TMPDIR, so what install.sh leaves behind there is
# only checked where mktemp uses it.
mkdir "$work/probe"
probe=$(TMPDIR="$work/probe" mktemp -d)
case $probe in
    "$work/probe/"*) tmpdir_used=1 ;;
    *) tmpdir_used=0 && rmdir "$probe" ;;
esac

# The stand-in for GitHub (server.py says what it serves), started on a free
# port.
releases="$work/releases"
mkdir -p "$releases" "$work/builds" "$work/sandbox"
"$python" "$here/server.py" "$releases" "$work/port" 2>"$work/server.log" &
server=$!
tries=0
until [ -s "$work/port" ]; do
    tries=$((tries + 1))
    if [ "$tries" -gt 300 ] || ! kill -0 "$server" 2>/dev/null; then
        echo "The stand-in for GitHub didn't start:" >&2
        cat "$work/server.log" >&2
        exit 1
    fi
    sleep 0.1
done
url="http://127.0.0.1:$(cat "$work/port")/releases"

sha256() {
    if command -v sha256sum >/dev/null 2>&1; then
        sha256sum "$@"
    else
        shasum -a 256 "$@"
    fi
}

# fake VERSION FILE writes a stand-in for steamcards VERSION to FILE: a
# script that answers --version the way steamcards does. VERSION "old" is a
# build from before --version, which doesn't know it.
fake() {
    mkdir -p "$(dirname "$2")"
    if [ "$1" = old ]; then
        cat >"$2" <<'EOF'
#!/bin/sh
# A stand-in for a steamcards from before --version, for install.sh's tests.
echo "error: unexpected argument '$1' found" >&2
exit 2
EOF
    else
        cat >"$2" <<EOF
#!/bin/sh
# A stand-in for steamcards $1, for install.sh's tests.
if [ "\${1:-}" = --version ]; then
    echo "steamcards $1"
fi
EOF
    fi
    chmod 755 "$2"
}

targets="aarch64-apple-darwin x86_64-apple-darwin x86_64-unknown-linux-musl aarch64-unknown-linux-musl"

# publish VERSION makes a release like the real ones: an archive for each
# macOS and Linux build (the binary, LICENSE and README.md), and SHA256SUMS.
# Its binary is kept in $work/builds/VERSION, to compare with what's
# installed.
publish() {
    build="$work/builds/$1"
    fake "$1" "$build/steamcards"
    echo "A stand-in for steamcards' licence." >"$build/LICENSE"
    mkdir -p "$releases/v$1"
    for target in $targets; do
        # A README of its own, so each archive has a checksum of its own.
        echo "A stand-in for steamcards $1 for $target." >"$build/README.md"
        COPYFILE_DISABLE=1 tar -czf "$releases/v$1/steamcards-$target.tar.gz" \
            -C "$build" steamcards LICENSE README.md
    done
    (cd "$releases/v$1" && sha256 steamcards-*.tar.gz) >"$releases/v$1/SHA256SUMS"
}

# latest VERSION chooses the release that /latest redirects to. "" is a
# repository with no releases, which GitHub redirects to its releases page,
# and "none" makes /latest a 404, like a private repository.
latest() {
    if [ "$1" = none ]; then
        rm -f "$releases/latest"
    else
        printf '%s\n' "$1" >"$releases/latest"
    fi
}

for version in 0.1.0 0.2.0 0.3.0 0.4.0 0.10.0; do
    publish "$version"
done
# 0.4.0's archives are swapped for 0.3.0's after its SHA256SUMS was written:
# real archives, which don't match their checksums.
for target in $targets; do
    cp "$releases/v0.3.0/steamcards-$target.tar.gz" "$releases/v0.4.0/"
done

# Each scenario starts with a sandbox of its own:
#   $home   its HOME
#   $bin    ~/.local/bin, the default install folder
#   $data   where steamcards keeps your sign-in and choices, in $home
#   $tmp    its TMPDIR
#   $path   its PATH: $bin, then the system's folders
# and the latest release at 0.2.0.
setup() {
    count=$((count + 1))
    sandbox="$work/sandbox/$count"
    home="$sandbox/home"
    bin="$home/.local/bin"
    if [ "$system" = Darwin ]; then
        data="$home/Library/Application Support/steamcards"
    else
        data="$home/.config/steamcards"
    fi
    tmp="$sandbox/tmp"
    mkdir -p "$home" "$tmp"
    path="$bin:$system_path"
    latest 0.2.0
    : >"$sandbox/out"
    : >"$sandbox/err"
    : >"$sandbox/tty"
    : >"$sandbox/ran"
    status="(didn't run)"
    problems=""
    skip=""
    log_start=$(($(wc -c <"$work/server.log") + 1))
}

# run_install [NAME=VALUE...] runs install.sh the way `curl ... | sh` does,
# piped into the shell under test, with the sandbox's HOME, PATH and TMPDIR,
# the stand-in's address, and any NAME=VALUE given, but nothing else from
# this environment.
run_install() {
    # A pipe, not a file, as with curl. And $shell is a command and its
    # arguments, like "/usr/bin/busybox sh", so it's split.
    # shellcheck disable=SC2002,SC2086
    cat "$installer" |
        env -i HOME="$home" PATH="$path" TMPDIR="$tmp" STEAMCARDS_RELEASES_URL="$url" "$@" \
            $shell >"$sandbox/out" 2>"$sandbox/err"
    status=$?
}

# run_uninstall [NAME=VALUE...] runs uninstall.sh the same way, with no
# terminal, as on CI, so there's nobody to ask.
run_uninstall() {
    # shellcheck disable=SC2002,SC2086
    cat "$uninstaller" |
        "$python" "$here/terminal.py" none \
            env -i HOME="$home" PATH="$path" TMPDIR="$tmp" "$@" \
            $shell >"$sandbox/out" 2>"$sandbox/err"
    status=$?
}

# answer_uninstall ANSWER [NAME=VALUE...] runs it with a terminal of its own,
# as when someone pastes the one-liner into Terminal, and types ANSWER to
# what it asks there. What it shows on the terminal is in $sandbox/tty.
answer_uninstall() {
    typed=$1
    shift
    # shellcheck disable=SC2002,SC2086
    cat "$uninstaller" |
        "$python" "$here/terminal.py" answer "$typed" "$sandbox/tty" \
            env -i HOME="$home" PATH="$path" TMPDIR="$tmp" "$@" \
            $shell >"$sandbox/out" 2>"$sandbox/err"
    status=$?
}

# fake_tool NAME FILE writes a stand-in for brew or cargo, called NAME, to
# FILE: a script that notes each command it's given in $sandbox/ran, and does
# nothing else, or fails if there's a $sandbox/fails. Asked for the taps
# (brew tap), it lists $sandbox/taps.
fake_tool() {
    mkdir -p "$(dirname "$2")"
    cat >"$2" <<EOF
#!/bin/sh
# A stand-in for $1, for uninstall.sh's tests.
echo "$1 \$*" >>"$sandbox/ran"
if [ -f "$sandbox/fails" ]; then
    echo "Error: a stand-in for $1, failing on purpose." >&2
    exit 1
fi
if [ "\$*" = tap ] && [ -f "$sandbox/taps" ]; then
    cat "$sandbox/taps"
fi
EOF
    chmod 755 "$2"
}

# fake_system NAME makes the scenario's system NAME, like Linux, for
# uninstall.sh: a stand-in for uname, first on its PATH.
fake_system() {
    mkdir -p "$sandbox/system"
    printf '#!/bin/sh\necho %s\n' "$1" >"$sandbox/system/uname"
    chmod 755 "$sandbox/system/uname"
    path="$sandbox/system:$path"
}

# homebrew_copy puts a steamcards in the sandbox the way Homebrew does: in
# its Cellar, linked from its bin folder, $sandbox/homebrew/bin.
homebrew_copy() {
    fake 0.1.0 "$sandbox/homebrew/Cellar/steamcards/0.1.0/bin/steamcards"
    mkdir -p "$sandbox/homebrew/bin"
    ln -s ../Cellar/steamcards/0.1.0/bin/steamcards "$sandbox/homebrew/bin/steamcards"
}

# add_data FOLDER puts sign-in and choices in FOLDER, as steamcards does,
# with a debug log beside them.
add_data() {
    mkdir -p "$1"
    echo '{"accounts": {}, "preferences": {}}' >"$1/config.json"
    echo "A stand-in for steamcards' debug log." >"$1/debug.log"
}

# on_system_path PROGRAM: there's a PROGRAM in the system's folders, which
# every scenario's PATH has.
on_system_path() {
    (PATH=$system_path && command -v "$1") >/dev/null 2>&1
}

problem() {
    problems="$problems
    $*"
}

# has_line FILE PATTERN: a line of FILE matches PATTERN, a shell pattern.
has_line() {
    while IFS= read -r line || [ -n "$line" ]; do
        # shellcheck disable=SC2254 # A pattern, so it isn't quoted.
        case $line in
            $2) return 0 ;;
        esac
    done <"$1"
    return 1
}

# has_prefix FILE TEXT: a line of FILE starts with TEXT, taken as it is.
has_prefix() {
    while IFS= read -r line || [ -n "$line" ]; do
        case $line in
            "$2"*) return 0 ;;
        esac
    done <"$1"
    return 1
}

expect_status() {
    [ "$status" = "$1" ] || problem "exit status $status, expected $1"
}

expect_out() {
    has_line "$sandbox/out" "$1" || problem "no line of output like: $1"
}

expect_no_out() {
    ! has_line "$sandbox/out" "$1" || problem "a line of output like: $1"
}

expect_err() {
    has_line "$sandbox/err" "$1" || problem "no line of error output like: $1"
}

expect_no_err() {
    [ ! -s "$sandbox/err" ] || problem "error output, where there should be none"
}

# expect_same FILE EXPECTED: FILE is a copy of EXPECTED.
expect_same() {
    cmp -s "$1" "$2" || problem "$1 isn't the same as $2"
}

# expect_version FILE VERSION: FILE is a steamcards that says it's VERSION.
expect_version() {
    said=$("$1" --version 2>/dev/null)
    [ "$said" = "steamcards $2" ] || problem "$1 says \"$said\", not \"steamcards $2\""
}

expect_missing() {
    if [ -e "$1" ] || [ -L "$1" ]; then
        problem "$1 is there, and shouldn't be"
    fi
}

expect_present() {
    if [ ! -e "$1" ] && [ ! -L "$1" ]; then
        problem "$1 isn't there, and should be"
    fi
}

# expect_asked QUESTION: uninstall.sh asked QUESTION on the terminal.
expect_asked() {
    has_prefix "$sandbox/tty" "$1" || problem "not asked on the terminal: $1"
}

expect_not_asked() {
    [ ! -s "$sandbox/tty" ] || problem "asked something on the terminal, where it shouldn't"
}

# expect_ran COMMAND...: the stand-ins for brew and cargo were given these
# commands, in this order, and no others.
expect_ran() {
    printf '%s\n' "$@" >"$sandbox/expected-ran"
    cmp -s "$sandbox/ran" "$sandbox/expected-ran" ||
        problem "brew or cargo wasn't run as expected: $*"
}

# expect_tidy DIR: install.sh left nothing behind, in DIR or its TMPDIR.
expect_tidy() {
    for leftover in "$1"/.steamcards.new.*; do
        if [ -e "$leftover" ]; then
            problem "left behind: $leftover"
        fi
    done
    if [ "$tmpdir_used" = 1 ] && [ -n "$(ls -A "$tmp")" ]; then
        problem "left behind in TMPDIR: $(ls -A "$tmp")"
    fi
}

expect_writable() {
    if [ "$(id -u)" = 0 ]; then
        skip="root can write to any folder"
        return 1
    fi
}

finish() {
    if [ -n "$skip" ]; then
        skipped=$((skipped + 1))
        echo "skip  $shell_name: $name ($skip)"
    elif [ -z "$problems" ]; then
        passed=$((passed + 1))
        echo "ok    $shell_name: $name"
    else
        failed=$((failed + 1))
        echo "FAIL  $shell_name: $name$problems"
        echo "    exit status: $status"
        sed 's/^/    out: /' "$sandbox/out"
        sed 's/^/    err: /' "$sandbox/err"
        sed 's/^/    terminal: /' "$sandbox/tty"
        sed 's/^/    ran: /' "$sandbox/ran"
        tail -c "+$log_start" "$work/server.log" | sed 's/^/    server: /'
    fi
}

# The scenarios, in the order they run: install.sh's, then uninstall.sh's.
scenarios="fresh_install path_hint up_to_date update downgrade forced old_build
elsewhere install_dir cargo cargo_home homebrew tampered unwritable
uncreatable no_release no_release_page newer numeric_order prerelease
missing_version trailing_slash
uninstall uninstall_keeps_data uninstall_asks_yes uninstall_asks_no
uninstall_delete_data uninstall_keep_data uninstall_nothing
uninstall_only_data uninstall_default_first uninstall_elsewhere
uninstall_homebrew uninstall_homebrew_untapped uninstall_homebrew_fails
uninstall_homebrew_no_brew uninstall_cargo uninstall_cargo_home
uninstall_no_cargo uninstall_unwritable uninstall_linux
uninstall_linux_relative uninstall_macos uninstall_no_home"

scenario_fresh_install() {
    name="a fresh install"
    run_install
    expect_status 0
    expect_out "Downloading steamcards 0.2.0 for *..."
    expect_out "Installed steamcards 0.2.0 to $bin/steamcards."
    expect_out "Run it with: steamcards"
    expect_no_out "*isn't on your PATH*"
    expect_no_err
    expect_same "$bin/steamcards" "$work/builds/0.2.0/steamcards"
    expect_version "$bin/steamcards" 0.2.0
    expect_tidy "$bin"
}

scenario_path_hint() {
    name="a fresh install to a folder that isn't on PATH, with the hint"
    path=$system_path
    run_install
    expect_status 0
    expect_out "Installed steamcards 0.2.0 to $bin/steamcards."
    expect_out "$bin isn't on your PATH yet. Add this line to your shell's startup file"
    expect_out "    export PATH=\"$bin:\$PATH\""
    expect_no_out "Run it with: steamcards"
    expect_same "$bin/steamcards" "$work/builds/0.2.0/steamcards"
}

scenario_up_to_date() {
    name="already up to date"
    fake 0.2.0 "$bin/steamcards"
    cp "$bin/steamcards" "$sandbox/before"
    run_install
    expect_status 0
    expect_out "steamcards 0.2.0 is already up to date."
    expect_no_out "Downloading*"
    expect_no_err
    expect_same "$bin/steamcards" "$sandbox/before"
}

scenario_update() {
    name="an update"
    fake 0.1.0 "$bin/steamcards"
    run_install
    expect_status 0
    expect_out "Updated steamcards 0.1.0 → 0.2.0."
    expect_no_out "Run it with: steamcards"
    expect_no_err
    expect_same "$bin/steamcards" "$work/builds/0.2.0/steamcards"
    expect_tidy "$bin"
}

scenario_downgrade() {
    name="a pinned downgrade, with STEAMCARDS_VERSION=v0.1.0"
    fake 0.2.0 "$bin/steamcards"
    run_install STEAMCARDS_VERSION=v0.1.0
    expect_status 0
    expect_out "Downloading steamcards 0.1.0 for *..."
    expect_out "Downgraded steamcards 0.2.0 → 0.1.0."
    expect_same "$bin/steamcards" "$work/builds/0.1.0/steamcards"
}

scenario_forced() {
    name="a forced reinstall, with STEAMCARDS_FORCE=1"
    fake 0.2.0 "$bin/steamcards"
    echo "# Built locally." >>"$bin/steamcards"
    run_install STEAMCARDS_FORCE=1
    expect_status 0
    expect_out "Reinstalled steamcards 0.2.0."
    expect_same "$bin/steamcards" "$work/builds/0.2.0/steamcards"
}

scenario_old_build() {
    name="an old build without --version, replaced"
    fake old "$bin/steamcards"
    run_install
    expect_status 0
    expect_out "Updated steamcards to 0.2.0."
    expect_same "$bin/steamcards" "$work/builds/0.2.0/steamcards"
}

scenario_elsewhere() {
    name="a copy elsewhere on PATH, updated where it is"
    fake 0.1.0 "$sandbox/tools/steamcards"
    path="$bin:$sandbox/tools:$system_path"
    run_install
    expect_status 0
    expect_out "Updated steamcards 0.1.0 → 0.2.0."
    expect_no_out "*isn't on your PATH*"
    expect_same "$sandbox/tools/steamcards" "$work/builds/0.2.0/steamcards"
    expect_missing "$bin/steamcards"
    expect_tidy "$sandbox/tools"
}

scenario_install_dir() {
    name="STEAMCARDS_INSTALL_DIR, which a copy on PATH doesn't change"
    fake 0.1.0 "$sandbox/tools/steamcards"
    path="$sandbox/tools:$system_path"
    run_install STEAMCARDS_INSTALL_DIR="$sandbox/chosen"
    expect_status 0
    expect_out "Installed steamcards 0.2.0 to $sandbox/chosen/steamcards."
    expect_out "$sandbox/chosen isn't on your PATH yet. *"
    expect_same "$sandbox/chosen/steamcards" "$work/builds/0.2.0/steamcards"
    expect_version "$sandbox/tools/steamcards" 0.1.0
}

scenario_cargo() {
    name="a copy that cargo installed, refused"
    fake 0.1.0 "$home/.cargo/bin/steamcards"
    cp "$home/.cargo/bin/steamcards" "$sandbox/before"
    path="$bin:$home/.cargo/bin:$system_path"
    run_install
    expect_status 1
    expect_err "steamcards install: $home/.cargo/bin/steamcards was built from source with cargo. Update it the same way (https://github.com/joshgallantt/steamcards/blob/main/CONTRIBUTING.md#building-from-source), or set STEAMCARDS_INSTALL_DIR to install a prebuilt copy somewhere else."
    expect_same "$home/.cargo/bin/steamcards" "$sandbox/before"
    expect_missing "$bin/steamcards"
}

scenario_cargo_home() {
    name="a copy that cargo installed in CARGO_HOME, refused"
    fake 0.1.0 "$sandbox/cargo/bin/steamcards"
    cp "$sandbox/cargo/bin/steamcards" "$sandbox/before"
    path="$bin:$sandbox/cargo/bin:$system_path"
    run_install CARGO_HOME="$sandbox/cargo"
    expect_status 1
    expect_err "steamcards install: $sandbox/cargo/bin/steamcards was built from source with cargo. *"
    expect_same "$sandbox/cargo/bin/steamcards" "$sandbox/before"
}

scenario_homebrew() {
    name="a copy that Homebrew installed, refused"
    fake 0.1.0 "$sandbox/homebrew/Cellar/steamcards/0.1.0/bin/steamcards"
    mkdir -p "$sandbox/homebrew/bin"
    ln -s ../Cellar/steamcards/0.1.0/bin/steamcards "$sandbox/homebrew/bin/steamcards"
    path="$bin:$sandbox/homebrew/bin:$system_path"
    run_install
    expect_status 1
    expect_err "steamcards install: $sandbox/homebrew/bin/steamcards came from Homebrew. Update it with: brew upgrade steamcards"
    [ -L "$sandbox/homebrew/bin/steamcards" ] || problem "Homebrew's link was replaced"
    expect_version "$sandbox/homebrew/bin/steamcards" 0.1.0
    expect_missing "$bin/steamcards"
}

scenario_tampered() {
    name="a tampered archive, refused, with nothing replaced"
    fake 0.1.0 "$bin/steamcards"
    cp "$bin/steamcards" "$sandbox/before"
    latest 0.4.0
    run_install
    expect_status 1
    expect_err "steamcards install: steamcards-*.tar.gz doesn't match its checksum in SHA256SUMS, so nothing was installed"
    expect_same "$bin/steamcards" "$sandbox/before"
    expect_tidy "$bin"
}

scenario_unwritable() {
    name="an install folder that can't be written to"
    expect_writable || return
    fake 0.1.0 "$bin/steamcards"
    cp "$bin/steamcards" "$sandbox/before"
    chmod 555 "$bin"
    run_install
    chmod 755 "$bin"
    expect_status 1
    expect_err "steamcards install: can't write to $bin, and this script doesn't use sudo. Set STEAMCARDS_INSTALL_DIR to a folder you own."
    expect_same "$bin/steamcards" "$sandbox/before"
}

scenario_uncreatable() {
    name="an install folder that can't be created"
    expect_writable || return
    mkdir -p "$sandbox/locked"
    chmod 555 "$sandbox/locked"
    run_install STEAMCARDS_INSTALL_DIR="$sandbox/locked/bin"
    chmod 755 "$sandbox/locked"
    expect_status 1
    expect_err "steamcards install: couldn't create $sandbox/locked/bin. Set STEAMCARDS_INSTALL_DIR to a folder you own."
    expect_missing "$sandbox/locked/bin"
}

scenario_no_release() {
    name="no release yet: /latest is a 404"
    latest none
    run_install
    expect_status 1
    expect_err "steamcards install: couldn't find a release at $url"
    expect_missing "$bin/steamcards"
}

scenario_no_release_page() {
    name="no release yet: /latest redirects to the releases page"
    latest ""
    run_install
    expect_status 1
    expect_err "steamcards install: couldn't find a release at $url"
    expect_missing "$bin/steamcards"
}

scenario_newer() {
    name="a copy newer than the latest release, left alone"
    fake 0.3.0 "$bin/steamcards"
    cp "$bin/steamcards" "$sandbox/before"
    run_install
    expect_status 0
    expect_out "steamcards 0.3.0 is newer than the latest release, 0.2.0. Leaving it as it is."
    expect_no_err
    expect_same "$bin/steamcards" "$sandbox/before"
}

scenario_numeric_order() {
    name="0.9.0 updated to 0.10.0, as numbers compare"
    fake 0.9.0 "$bin/steamcards"
    latest 0.10.0
    run_install
    expect_status 0
    expect_out "Updated steamcards 0.9.0 → 0.10.0."
    expect_same "$bin/steamcards" "$work/builds/0.10.0/steamcards"
}

scenario_prerelease() {
    name="a pre-release, updated to its release"
    fake 0.2.0-rc.1 "$bin/steamcards"
    run_install
    expect_status 0
    expect_out "Updated steamcards 0.2.0-rc.1 → 0.2.0."
    expect_same "$bin/steamcards" "$work/builds/0.2.0/steamcards"
}

scenario_missing_version() {
    name="a pinned version that doesn't exist"
    fake 0.1.0 "$bin/steamcards"
    cp "$bin/steamcards" "$sandbox/before"
    run_install STEAMCARDS_VERSION=9.9.9
    expect_status 1
    expect_err "steamcards install: couldn't download $url/download/v9.9.9/steamcards-*.tar.gz"
    expect_same "$bin/steamcards" "$sandbox/before"
    expect_tidy "$bin"
}

scenario_trailing_slash() {
    name="STEAMCARDS_RELEASES_URL ending in /"
    run_install STEAMCARDS_RELEASES_URL="$url/"
    expect_status 0
    expect_out "Installed steamcards 0.2.0 to $bin/steamcards."
    expect_same "$bin/steamcards" "$work/builds/0.2.0/steamcards"
}

scenario_uninstall() {
    name="uninstall: the copy in ~/.local/bin, deleted"
    fake 0.2.0 "$bin/steamcards"
    run_uninstall
    expect_status 0
    expect_out "Deleting $bin/steamcards."
    expect_out "Uninstalled steamcards."
    expect_no_err
    expect_missing "$bin/steamcards"
    expect_present "$bin"
}

scenario_uninstall_keeps_data() {
    name="uninstall: your sign-in and choices kept, with no terminal to ask in"
    fake 0.2.0 "$bin/steamcards"
    add_data "$data"
    run_uninstall
    expect_status 0
    expect_out "Uninstalled steamcards. Your sign-in and choices are still in $data."
    expect_no_err
    expect_missing "$bin/steamcards"
    expect_present "$data/config.json"
}

scenario_uninstall_asks_yes() {
    name="uninstall: asks on the terminal, and y deletes your sign-in and choices"
    fake 0.2.0 "$bin/steamcards"
    add_data "$data"
    answer_uninstall y
    expect_status 0
    expect_asked "Also delete your sign-in and choices? [y/N] "
    expect_out "Uninstalled steamcards and deleted your sign-in and choices."
    expect_no_err
    expect_missing "$bin/steamcards"
    expect_missing "$data"
    expect_present "$(dirname "$data")"
}

scenario_uninstall_asks_no() {
    name="uninstall: asks on the terminal, and just Enter keeps them"
    fake 0.2.0 "$bin/steamcards"
    add_data "$data"
    answer_uninstall ""
    expect_status 0
    expect_asked "Also delete your sign-in and choices? [y/N] "
    expect_out "Uninstalled steamcards. Your sign-in and choices are still in $data."
    expect_missing "$bin/steamcards"
    expect_present "$data/config.json"
}

scenario_uninstall_delete_data() {
    name="uninstall: STEAMCARDS_DELETE_DATA=1 deletes them, with nothing else in that folder"
    fake 0.2.0 "$bin/steamcards"
    add_data "$data"
    # Another app's, beside them.
    add_data "$(dirname "$data")/another-app"
    run_uninstall STEAMCARDS_DELETE_DATA=1
    expect_status 0
    expect_out "Uninstalled steamcards and deleted your sign-in and choices."
    expect_no_err
    expect_missing "$data"
    expect_present "$(dirname "$data")/another-app/config.json"
}

scenario_uninstall_keep_data() {
    name="uninstall: STEAMCARDS_DELETE_DATA=0 keeps them, without asking"
    fake 0.2.0 "$bin/steamcards"
    add_data "$data"
    answer_uninstall y STEAMCARDS_DELETE_DATA=0
    expect_status 0
    expect_not_asked
    expect_out "Uninstalled steamcards. Your sign-in and choices are still in $data."
    expect_present "$data/config.json"
}

scenario_uninstall_nothing() {
    name="uninstall: nothing installed"
    run_uninstall
    expect_status 0
    expect_out "steamcards isn't installed, so there's nothing to remove."
    expect_no_err
}

scenario_uninstall_only_data() {
    name="uninstall: nothing installed, but sign-in and choices, still offered"
    add_data "$data"
    answer_uninstall y
    expect_status 0
    expect_out "steamcards isn't installed."
    expect_asked "Delete your sign-in and choices? [y/N] "
    expect_out "Deleted your sign-in and choices."
    expect_no_err
    expect_missing "$data"
}

scenario_uninstall_default_first() {
    name="uninstall: the copy in ~/.local/bin, before one that's first on PATH"
    fake 0.2.0 "$bin/steamcards"
    fake 0.1.0 "$sandbox/tools/steamcards"
    path="$sandbox/tools:$bin:$system_path"
    run_uninstall
    expect_status 0
    expect_out "Deleting $bin/steamcards."
    expect_missing "$bin/steamcards"
    expect_version "$sandbox/tools/steamcards" 0.1.0
}

scenario_uninstall_elsewhere() {
    name="uninstall: a copy elsewhere on PATH, deleted there"
    fake 0.1.0 "$sandbox/tools/steamcards"
    echo "Another program." >"$sandbox/tools/another-program"
    path="$bin:$sandbox/tools:$system_path"
    run_uninstall
    expect_status 0
    expect_out "Deleting $sandbox/tools/steamcards."
    expect_out "Uninstalled steamcards."
    expect_missing "$sandbox/tools/steamcards"
    expect_present "$sandbox/tools/another-program"
}

scenario_uninstall_homebrew() {
    name="uninstall: a copy from Homebrew, uninstalled and untapped with its own brew"
    homebrew_copy
    fake_tool brew "$sandbox/homebrew/bin/brew"
    printf '%s\n' homebrew/cask joshgallantt/steamcards >"$sandbox/taps"
    # Another Homebrew's brew, first on PATH, as on a Mac with two.
    fake_tool "the other brew" "$sandbox/other/bin/brew"
    path="$sandbox/other/bin:$bin:$sandbox/homebrew/bin:$system_path"
    add_data "$data"
    run_uninstall STEAMCARDS_DELETE_DATA=1
    expect_status 0
    expect_out "steamcards came from Homebrew. Uninstalling it with: brew uninstall steamcards"
    expect_out "Uninstalled steamcards and deleted your sign-in and choices."
    expect_no_err
    expect_ran "brew uninstall steamcards" "brew tap" "brew untap joshgallantt/steamcards"
    # Homebrew's to remove, not the script's.
    [ -L "$sandbox/homebrew/bin/steamcards" ] || problem "Homebrew's link was deleted"
    expect_missing "$data"
}

scenario_uninstall_homebrew_untapped() {
    name="uninstall: a copy from Homebrew without the tap, with brew found on PATH"
    homebrew_copy
    fake_tool brew "$sandbox/tools/brew"
    printf '%s\n' homebrew/cask joshgallantt/steamcards-beta >"$sandbox/taps"
    path="$bin:$sandbox/homebrew/bin:$sandbox/tools:$system_path"
    run_uninstall
    expect_status 0
    expect_out "Uninstalled steamcards."
    expect_no_err
    expect_ran "brew uninstall steamcards" "brew tap"
}

scenario_uninstall_homebrew_fails() {
    name="uninstall: a copy from Homebrew that brew can't uninstall, with nothing else deleted"
    homebrew_copy
    fake_tool brew "$sandbox/homebrew/bin/brew"
    : >"$sandbox/fails"
    path="$bin:$sandbox/homebrew/bin:$system_path"
    add_data "$data"
    run_uninstall STEAMCARDS_DELETE_DATA=1
    expect_status 1
    expect_err "steamcards uninstall: brew couldn't uninstall steamcards"
    expect_ran "brew uninstall steamcards"
    expect_no_out "Uninstalled*"
    expect_present "$data/config.json"
}

scenario_uninstall_homebrew_no_brew() {
    name="uninstall: a copy from Homebrew, with no brew to be found, refused"
    if on_system_path brew; then
        skip="there's a brew in $system_path"
        return
    fi
    homebrew_copy
    path="$bin:$sandbox/homebrew/bin:$system_path"
    add_data "$data"
    run_uninstall STEAMCARDS_DELETE_DATA=1
    expect_status 1
    expect_err "steamcards uninstall: steamcards came from Homebrew, but brew isn't on your PATH. Uninstall it with: brew uninstall steamcards && brew untap joshgallantt/steamcards"
    [ -L "$sandbox/homebrew/bin/steamcards" ] || problem "Homebrew's link was deleted"
    expect_present "$data/config.json"
}

scenario_uninstall_cargo() {
    name="uninstall: a copy that cargo installed, uninstalled with cargo"
    fake 0.1.0 "$home/.cargo/bin/steamcards"
    fake_tool cargo "$home/.cargo/bin/cargo"
    cp "$home/.cargo/bin/steamcards" "$sandbox/before"
    path="$bin:$home/.cargo/bin:$system_path"
    run_uninstall
    expect_status 0
    expect_out "steamcards was built from source with cargo. Uninstalling it with: cargo uninstall steamcards"
    expect_out "Uninstalled steamcards."
    expect_no_err
    expect_ran "cargo uninstall steamcards"
    # cargo's to remove, so that it forgets it too.
    expect_same "$home/.cargo/bin/steamcards" "$sandbox/before"
}

scenario_uninstall_cargo_home() {
    name="uninstall: a copy that cargo installed in CARGO_HOME"
    fake 0.1.0 "$sandbox/cargo/bin/steamcards"
    fake_tool cargo "$sandbox/rust/bin/cargo"
    path="$bin:$sandbox/cargo/bin:$sandbox/rust/bin:$system_path"
    run_uninstall CARGO_HOME="$sandbox/cargo"
    expect_status 0
    expect_out "Uninstalled steamcards."
    expect_ran "cargo uninstall steamcards"
    expect_version "$sandbox/cargo/bin/steamcards" 0.1.0
}

scenario_uninstall_no_cargo() {
    name="uninstall: a copy that cargo installed, with no cargo to be found, refused"
    if on_system_path cargo; then
        skip="there's a cargo in $system_path"
        return
    fi
    fake 0.1.0 "$home/.cargo/bin/steamcards"
    path="$bin:$home/.cargo/bin:$system_path"
    run_uninstall
    expect_status 1
    expect_err "steamcards uninstall: steamcards was built from source with cargo, but cargo isn't on your PATH. Uninstall it with: cargo uninstall steamcards"
    expect_version "$home/.cargo/bin/steamcards" 0.1.0
}

scenario_uninstall_unwritable() {
    name="uninstall: a copy in a folder you can't change, refused"
    expect_writable || return
    fake 0.1.0 "$sandbox/locked/steamcards"
    path="$bin:$sandbox/locked:$system_path"
    chmod 555 "$sandbox/locked"
    run_uninstall
    chmod 755 "$sandbox/locked"
    expect_status 1
    expect_err "steamcards uninstall: steamcards is in $sandbox/locked, which only an administrator can change, and this script doesn't use sudo. Delete it with: sudo rm \"$sandbox/locked/steamcards\""
    expect_version "$sandbox/locked/steamcards" 0.1.0
}

scenario_uninstall_linux() {
    name="uninstall: on Linux, sign-in and choices in XDG_CONFIG_HOME when it's set"
    fake_system Linux
    add_data "$home/.config/steamcards"
    add_data "$sandbox/xdg/steamcards"
    run_uninstall XDG_CONFIG_HOME="$sandbox/xdg" STEAMCARDS_DELETE_DATA=1
    expect_status 0
    expect_out "Deleted your sign-in and choices."
    expect_missing "$sandbox/xdg/steamcards"
    expect_present "$home/.config/steamcards/config.json"
}

scenario_uninstall_linux_relative() {
    name="uninstall: on Linux, an XDG_CONFIG_HOME that isn't a full path, which steamcards ignores"
    fake_system Linux
    add_data "$home/.config/steamcards"
    run_uninstall XDG_CONFIG_HOME=not-a-full-path STEAMCARDS_DELETE_DATA=1
    expect_status 0
    expect_out "Deleted your sign-in and choices."
    expect_missing "$home/.config/steamcards"
}

scenario_uninstall_macos() {
    name="uninstall: on macOS, sign-in and choices in Application Support, whatever XDG_CONFIG_HOME says"
    fake_system Darwin
    add_data "$home/Library/Application Support/steamcards"
    add_data "$sandbox/xdg/steamcards"
    run_uninstall XDG_CONFIG_HOME="$sandbox/xdg" STEAMCARDS_DELETE_DATA=1
    expect_status 0
    expect_out "Deleted your sign-in and choices."
    expect_missing "$home/Library/Application Support/steamcards"
    expect_present "$sandbox/xdg/steamcards/config.json"
}

scenario_uninstall_no_home() {
    name="uninstall: no HOME, refused"
    fake 0.2.0 "$bin/steamcards"
    run_uninstall HOME=
    expect_status 1
    expect_err "steamcards uninstall: HOME isn't set, so there's no telling where steamcards is"
    expect_version "$bin/steamcards" 0.2.0
}

echo "Testing install.sh against a stand-in for GitHub at $url, and uninstall.sh, under: $shells"
count=0
passed=0
failed=0
skipped=0
for shell_name in $shells; do
    case $shell_name in
        busybox) invocation="busybox sh" ;;
        *) invocation=$shell_name ;;
    esac
    # The shell's full path, as the scenarios' PATH may not have it.
    program=${invocation%% *}
    if ! found=$(command -v "$program"); then
        echo "FAIL  $shell_name: there's no $program here"
        failed=$((failed + 1))
        continue
    fi
    shell="$found${invocation#"$program"}"
    for scenario in $scenarios; do
        setup
        "scenario_$scenario"
        finish
    done
done

echo ""
echo "$passed passed, $failed failed, $skipped skipped."
if [ "$failed" -gt 0 ]; then
    exit 1
fi
