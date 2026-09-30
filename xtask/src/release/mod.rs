//! `cargo xtask release`: publishing a release, from the version bump to
//! Homebrew, with one question on the way.
//!
//! 1. **Checks** a release can be made: the GitHub CLI is signed in, and this
//!    is `main` with nothing uncommitted, level with GitHub.
//! 2. **Prepares** it here: works out the version, bumps `Cargo.toml` and
//!    `Cargo.lock`, runs every CI check, then commits "Release vX.Y.Z" and
//!    tags it.
//! 3. **Asks**, then **publishes**: pushes `main` and the tag together.
//!    Pushing `main` runs the tests on the release commit, and the tag starts
//!    `.github/workflows/release.yml`, which waits for them to pass,
//!    builds steamcards for macOS and Linux, checks each build runs,
//!    and publishes them as a GitHub release. GitHub writes its notes from
//!    the pull requests merged since the last release.
//! 4. **Follows that build** to the end, then points the Homebrew formula at
//!    the release and pushes that to `main` (the workflow never pushes to
//!    `main`: only the maintainer does), and says how to install the release.
//!
//! Nothing leaves this machine before the question in step 3, and the same
//! command run again carries on from wherever it stopped.
//!
//! Two smaller commands go with it: `cargo xtask release-notes <tag>` prints
//! the release workflow's part of a release's notes (how to install it), and
//! `cargo xtask homebrew <tag>` points the Homebrew formula at a published
//! release.

mod homebrew;
mod notes;
mod version;

use std::{
    cmp::Ordering,
    fs,
    io::{self, BufRead, Write as _},
    path::Path,
    process::Command,
    thread,
    time::{Duration, Instant},
};

use homebrew::{FORMULA, Pointed};
use serde_json::Value;
use version::{Bump, Version};

const USAGE: &str = "usage: cargo xtask release <patch | minor | major | X.Y.Z> [--yes]

  patch, minor or major works the next version out from Cargo.toml's, or give
  one, like 0.2.0 or 0.2.0-rc.1. It asks before it publishes anything, unless
  given --yes.";

const WORKFLOW: &str = "release.yml";

/// How long GitHub gets to start the release build once the tag is pushed.
const BUILD_START: Duration = Duration::from_secs(120);

/// What a release commit changes.
const RELEASE_FILES: [&str; 2] = ["Cargo.toml", "Cargo.lock"];

const NO_VERSION: &str = "Cargo.toml has no version in [workspace.package]";
const NO_REPOSITORY: &str = "Cargo.toml has no repository in [workspace.package]";

pub(crate) fn run(root: &Path, args: &[String]) -> Result<(), String> {
    let (wanted, yes) = arguments(args)?;
    let github = check(root)?;

    let manifest = read(&root.join("Cargo.toml"))?;
    let current = Version::parse(field(&manifest, "version").ok_or(NO_VERSION)?.1)?;
    let version = match wanted {
        Wanted::Exactly(version) => version,
        Wanted::Bump(bump) => {
            let unfinished = format!("v{current}");
            if tagged_here(root, &unfinished) && !tagged_on_github(root, &unfinished)? {
                return Err(format!(
                    "{unfinished} is tagged here but was never published. Finish it with \
                     `cargo xtask release {current}`, or drop it with `git tag -d {unfinished}`."
                ));
            }
            current.bumped(bump)
        }
    };
    let release = Release {
        root,
        tag: format!("v{version}"),
        version,
        current,
        yes,
    };

    if tagged_here(root, &release.tag) {
        println!(
            "• {} is tagged here already: carrying on from there.",
            release.tag
        );
        if !succeeds(
            root,
            "git",
            &["merge-base", "--is-ancestor", &release.tag, "HEAD"],
        ) {
            return Err(format!("{} is on a commit that isn't on main", release.tag));
        }
    } else {
        release.prepare()?;
    }

    if tagged_on_github(root, &release.tag)? {
        println!("• {} is on GitHub already.", release.tag);
    } else if !release.publish(&github)? {
        return Ok(());
    }

    let page = release.follow_build(&github)?;
    release.check_homebrew()?;
    release.report(&github, &page);
    Ok(())
}

/// `cargo xtask release-notes <tag> [--attested]`: the release workflow's part
/// of a release's notes, how to install it, which GitHub puts above the pull
/// requests it lists. `--attested` adds how to check GitHub's attestation of a
/// download.
pub(crate) fn print_notes(root: &Path, args: &[String]) -> Result<(), String> {
    const NOTES_USAGE: &str = "usage: cargo xtask release-notes <tag> [--attested]";
    let (tag, attested) = match args {
        [tag] => (tag, false),
        [tag, flag] if flag == "--attested" => (tag, true),
        _ => return Err(NOTES_USAGE.into()),
    };
    let version = Version::parse(tag.strip_prefix('v').ok_or(NOTES_USAGE)?)?;
    let manifest = read(&root.join("Cargo.toml"))?;
    let repository = field(&manifest, "repository").ok_or(NO_REPOSITORY)?.1;
    print!("{}", notes::release_notes(&version, repository, attested));
    Ok(())
}

/// `cargo xtask homebrew <tag>`: points the Homebrew formula at a published
/// release, unless it's on a newer one.
pub(crate) fn point_homebrew(root: &Path, args: &[String]) -> Result<(), String> {
    let [tag] = args else {
        return Err("usage: cargo xtask homebrew <tag>, like v0.2.0".into());
    };
    match homebrew::point(root, tag)? {
        Pointed::Changed => println!("✓ Pointed {FORMULA} at {tag}."),
        Pointed::Unchanged => println!("✓ {FORMULA} already points at {tag}."),
        Pointed::Newer(newer) => {
            println!("• {FORMULA} is on {newer}, newer than {tag}, so it stays there.");
        }
    }
    Ok(())
}

/// The version asked for.
enum Wanted {
    Exactly(Version),
    Bump(Bump),
}

fn arguments(args: &[String]) -> Result<(Wanted, bool), String> {
    let mut wanted = None;
    let mut yes = false;
    for arg in args {
        match arg.as_str() {
            "--yes" | "-y" => yes = true,
            word if !word.starts_with('-') && wanted.is_none() => {
                wanted = Some(match Bump::from_word(word) {
                    Some(bump) => Wanted::Bump(bump),
                    None => {
                        Wanted::Exactly(Version::parse(word.strip_prefix('v').unwrap_or(word))?)
                    }
                });
            }
            _ => return Err(USAGE.into()),
        }
    }
    Ok((wanted.ok_or(USAGE)?, yes))
}

/// What `gh` says about the repository.
struct GitHub {
    /// `owner/name`.
    name: String,
    private: bool,
}

/// 1. The GitHub CLI is ready, and this checkout can be released.
fn check(root: &Path) -> Result<GitHub, String> {
    if !succeeds(root, "gh", &["auth", "status"]) {
        return Err(
            "a release needs the GitHub CLI, signed in: install it from \
                    https://cli.github.com, then run `gh auth login`"
                .into(),
        );
    }
    let about = output(
        root,
        "gh",
        &[
            "repo",
            "view",
            "--json",
            "nameWithOwner,visibility",
            "--jq",
            r#".nameWithOwner + " " + .visibility"#,
        ],
    )?;
    let (name, visibility) = about
        .split_once(' ')
        .ok_or_else(|| format!("gh said {about:?} about the repository"))?;

    let branch = output(root, "git", &["rev-parse", "--abbrev-ref", "HEAD"])?;
    if branch != "main" {
        return Err(format!("releases are made from main, and this is {branch}"));
    }
    if !output(
        root,
        "git",
        &["status", "--porcelain", "--untracked-files=no"],
    )?
    .is_empty()
    {
        return Err("there are uncommitted changes: commit them first".into());
    }
    println!("• Fetching from GitHub…");
    output(root, "git", &["fetch", "--quiet", "--tags", "origin"])?;
    let behind = output(root, "git", &["rev-list", "--count", "HEAD..origin/main"])?;
    if behind != "0" {
        return Err(format!(
            "main is {behind} commit(s) behind GitHub: pull them first"
        ));
    }
    Ok(GitHub {
        name: name.to_owned(),
        private: visibility == "PRIVATE",
    })
}

struct Release<'a> {
    root: &'a Path,
    version: Version,
    /// `v` and the version: `v0.2.0`.
    tag: String,
    /// Cargo.toml's version before this release.
    current: Version,
    /// Publish without asking.
    yes: bool,
}

impl Release<'_> {
    /// 2. Bumps the version, runs every check, then commits and tags it.
    fn prepare(&self) -> Result<(), String> {
        let bump = match self.version.precedence(&self.current) {
            Ordering::Less => {
                return Err(format!(
                    "{} is older than Cargo.toml's version, {}",
                    self.version, self.current
                ));
            }
            Ordering::Equal => false,
            Ordering::Greater => true,
        };
        self.check_something_changed()?;

        if bump {
            println!("• Releasing {}, after {}.", self.version, self.current);
        } else {
            println!("• Releasing {}, the version Cargo.toml has.", self.version);
        }
        if bump {
            let manifest_path = self.root.join("Cargo.toml");
            let manifest = read(&manifest_path)?;
            let (line, _) = field(&manifest, "version").ok_or(NO_VERSION)?;
            let bumped = set_line(&manifest, line, &format!("version = \"{}\"", self.version));
            write(&manifest_path, &bumped)?;
            // Cargo.lock records every crate's version too.
            step(
                self.root,
                crate::CARGO,
                &["update", "--workspace", "--offline", "--quiet"],
            )?;
        }

        println!("• Running every CI check…");
        if !crate::run(&crate::CI) {
            if bump {
                let mut restore = vec!["checkout", "--"];
                restore.extend(RELEASE_FILES);
                output(self.root, "git", &restore)?;
            }
            return Err("a check failed, so nothing was committed or tagged".into());
        }

        if bump {
            let message = format!("Release {}", self.tag);
            let mut commit = vec!["commit", "--quiet", "-m", message.as_str(), "--"];
            commit.extend(RELEASE_FILES);
            step(self.root, "git", &commit)?;
        }
        let annotation = format!("steamcards {}", self.version);
        output(
            self.root,
            "git",
            &["tag", "--annotate", &self.tag, "-m", &annotation],
        )?;
        println!("✓ Tagged {}. Nothing has left this machine yet.", self.tag);
        Ok(())
    }

    /// Refuses a release with nothing in it: no commits since the last one,
    /// beyond the releases' own bookkeeping.
    fn check_something_changed(&self) -> Result<(), String> {
        let Ok(last) = output(
            self.root,
            "git",
            &["describe", "--tags", "--abbrev=0", "--match", "v*"],
        ) else {
            // The first release.
            return Ok(());
        };
        let range = format!("{last}..HEAD");
        let log = output(
            self.root,
            "git",
            &["log", "--no-merges", "--format=%s", &range],
        )?;
        if worth_noting(&log).is_empty() {
            return Err(format!("nothing has changed since {last}"));
        }
        Ok(())
    }

    /// 3. Asks, then pushes `main` and the tag. False if the answer was no.
    fn publish(&self, github: &GitHub) -> Result<bool, String> {
        let commits = output(self.root, "git", &["log", "--oneline", "origin/main..HEAD"])?;
        println!(
            "\nReady to publish steamcards {}. This pushes to GitHub:",
            self.version
        );
        if commits.is_empty() {
            println!("  - the tag {} (main is there already)", self.tag);
        } else {
            println!("  - main, with {} new commit(s):", commits.lines().count());
            for commit in commits.lines() {
                println!("      {commit}");
            }
            println!("  - the tag {}", self.tag);
        }
        println!(
            "  The tag starts the release build, which publishes steamcards {} for\n  \
             macOS and Linux on {}'s releases page.",
            self.version, github.name
        );
        if github.private {
            println!(
                "  The repository is private: only you and its collaborators can see the\n  \
                 release, and the install commands work for no one else until it's public."
            );
        }

        if !self.yes && !ask("Publish it?")? {
            println!("\nNothing was pushed. Run this command again to publish.");
            let head = output(self.root, "git", &["log", "-1", "--format=%s"])?;
            let reset = if head == format!("Release {}", self.tag) {
                " && git reset --keep HEAD~1"
            } else {
                ""
            };
            println!(
                "To undo the release instead: git tag -d {}{reset}",
                self.tag
            );
            return Ok(false);
        }

        // Both or neither: a tag without its commit on GitHub, or the other way
        // round, is a mess to clean up.
        let tag = format!("refs/tags/{}", self.tag);
        step(
            self.root,
            "git",
            &["push", "--atomic", "origin", "main", &tag],
        )?;
        Ok(true)
    }

    /// 4. Follows the release build to the end, and returns the release's page.
    fn follow_build(&self, github: &GitHub) -> Result<String, String> {
        println!("\n• Waiting for GitHub to start the release build…");
        // The build for this tag's commit: a tag that was moved to a fix can
        // have an older, failed build under the same name.
        let commit = output(self.root, "git", &["rev-list", "-n", "1", &self.tag])?;
        let asked = Instant::now();
        let run = loop {
            let listed = output(
                self.root,
                "gh",
                &[
                    "run",
                    "list",
                    "--workflow",
                    WORKFLOW,
                    "--branch",
                    &self.tag,
                    "--commit",
                    &commit,
                    "--limit",
                    "1",
                    "--json",
                    "databaseId,status,conclusion,url",
                ],
            )?;
            let runs: Value = serde_json::from_str(&listed).map_err(|e| e.to_string())?;
            if let Some(run) = runs.get(0) {
                break run.clone();
            }
            if asked.elapsed() > BUILD_START {
                return Err(format!(
                    "GitHub hasn't started a release build for {} after {}s: see \
                     https://github.com/{}/actions",
                    self.tag,
                    BUILD_START.as_secs(),
                    github.name
                ));
            }
            thread::sleep(Duration::from_secs(5));
        };

        let url = run["url"].as_str().unwrap_or_default();
        println!(
            "• Following the build: {url}\n  \
             (Ctrl-C is safe: the release carries on without this, and running this\n  \
             command again picks it back up.)"
        );
        let succeeded = if run["status"] == "completed" {
            run["conclusion"] == "success"
        } else {
            let id = run["databaseId"]
                .as_u64()
                .ok_or("gh listed a run with no id")?
                .to_string();
            succeeds_loudly(
                self.root,
                "gh",
                &["run", "watch", &id, "--exit-status", "--interval", "10"],
            )
        };

        let page = output(
            self.root,
            "gh",
            &[
                "release", "view", &self.tag, "--json", "url", "--jq", ".url",
            ],
        );
        match (page, succeeded) {
            (Ok(page), true) => Ok(page),
            (Ok(page), false) => {
                println!("! The release is published, but a later step of its build failed: {url}");
                Ok(page)
            }
            (Err(_), _) => Err(format!(
                "the release build failed before publishing anything: {url}\n  \
                 Fix it on main and release a new version, or move the tag to the fix:\n    \
                 git push origin :refs/tags/{tag} && git tag -d {tag}\n  \
                 then run this command again.",
                tag = self.tag
            )),
        }
    }

    /// The release workflow points Homebrew at the release. This makes sure it
    /// did, and does it from here if not. Pre-releases stay out of Homebrew.
    fn check_homebrew(&self) -> Result<(), String> {
        if self.version.is_pre_release() {
            println!(
                "• {} is a pre-release, so Homebrew stays on the last release.",
                self.tag
            );
            return Ok(());
        }
        // Anything merged on GitHub while the release built comes first.
        step(
            self.root,
            "git",
            &["pull", "--quiet", "--ff-only", "origin", "main"],
        )?;
        match homebrew::point(self.root, &self.tag)? {
            Pointed::Unchanged => println!("✓ Homebrew has {}.", self.version),
            Pointed::Newer(newer) => {
                println!("• Homebrew is on {newer}, newer than this, so it stays there.");
            }
            Pointed::Changed => {
                println!("• Pointing Homebrew at {}.", self.tag);
                let message = format!("Point the Homebrew formula at {}", self.tag);
                step(
                    self.root,
                    "git",
                    &["commit", "--quiet", "-m", &message, "--", FORMULA],
                )?;
                step(self.root, "git", &["push", "--quiet", "origin", "main"])?;
                println!("✓ Homebrew has {}.", self.version);
            }
        }
        Ok(())
    }

    fn report(&self, github: &GitHub, page: &str) {
        let raw = format!("https://raw.githubusercontent.com/{}/main", github.name);
        println!("\n✓ steamcards {} is released: {page}", self.version);
        if self.version.is_pre_release() {
            println!(
                "  A pre-release: the install scripts install it only when asked for by name:"
            );
            println!(
                "  curl -fsSL {raw}/install/install.sh | STEAMCARDS_VERSION={} sh",
                self.version
            );
        } else {
            println!("  macOS and Linux: curl -fsSL {raw}/install/install.sh | sh");
            println!("  Homebrew:        brew upgrade steamcards");
        }
        if github.private {
            println!("  The repository is private, so these work for you and collaborators only.");
        }
    }
}

/// The commit subjects that are changes, not the releases' own bookkeeping.
fn worth_noting(subjects: &str) -> Vec<&str> {
    subjects
        .lines()
        .filter(|s| {
            !s.starts_with("Release v") && !s.starts_with("Point the Homebrew formula at v")
        })
        .collect()
}

fn tagged_here(root: &Path, tag: &str) -> bool {
    succeeds(
        root,
        "git",
        &[
            "rev-parse",
            "--quiet",
            "--verify",
            &format!("refs/tags/{tag}"),
        ],
    )
}

fn tagged_on_github(root: &Path, tag: &str) -> Result<bool, String> {
    let found = output(
        root,
        "git",
        &["ls-remote", "--tags", "origin", &format!("refs/tags/{tag}")],
    )?;
    Ok(!found.is_empty())
}

/// Runs a command in the checkout and returns what it printed, trimmed.
fn output(root: &Path, program: &str, args: &[&str]) -> Result<String, String> {
    let out = Command::new(program)
        .args(args)
        .current_dir(root)
        .output()
        .map_err(|e| format!("couldn't run {program}: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "`{program} {}` failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_owned())
}

/// Runs a command in the checkout, with its output going to the terminal.
fn step(root: &Path, program: &str, args: &[&str]) -> Result<(), String> {
    if succeeds_loudly(root, program, args) {
        Ok(())
    } else {
        Err(format!("`{program} {}` failed", args.join(" ")))
    }
}

fn succeeds_loudly(root: &Path, program: &str, args: &[&str]) -> bool {
    Command::new(program)
        .args(args)
        .current_dir(root)
        .status()
        .is_ok_and(|s| s.success())
}

/// Whether a command succeeds, with its output hidden.
fn succeeds(root: &Path, program: &str, args: &[&str]) -> bool {
    Command::new(program)
        .args(args)
        .current_dir(root)
        .output()
        .is_ok_and(|out| out.status.success())
}

/// Asks a yes-or-no question on the terminal. Anything but yes is no.
fn ask(question: &str) -> Result<bool, String> {
    print!("\n{question} [y/N] ");
    io::stdout().flush().map_err(|e| e.to_string())?;
    let mut answer = String::new();
    io::stdin()
        .lock()
        .read_line(&mut answer)
        .map_err(|e| e.to_string())?;
    Ok(matches!(answer.trim(), "y" | "Y" | "yes" | "Yes"))
}

fn read(path: &Path) -> Result<String, String> {
    fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))
}

fn write(path: &Path, text: &str) -> Result<(), String> {
    fs::write(path, text).map_err(|e| format!("{}: {e}", path.display()))
}

/// A value in Cargo.toml's `[workspace.package]`, and its line.
fn field<'a>(toml: &'a str, key: &str) -> Option<(usize, &'a str)> {
    let mut section = "";
    for (i, line) in toml.lines().enumerate() {
        let line = line.trim();
        if line.starts_with('[') {
            section = line;
        } else if section == "[workspace.package]"
            && let Some(value) = line
                .strip_prefix(key)
                .and_then(|rest| rest.trim_start().strip_prefix('='))
        {
            return Some((i, value.trim().trim_matches('"')));
        }
    }
    None
}

/// `text` with line `index` replaced by `new`.
fn set_line(text: &str, index: usize, new: &str) -> String {
    let mut lines: Vec<&str> = text.lines().collect();
    lines[index] = new;
    let mut out = lines.join("\n");
    if text.ends_with('\n') {
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn workspace_fields_are_found_in_their_section_only() {
        let toml = "[workspace]\nmembers = [\"app\"]\n\n[workspace.package]\n\
                    edition = \"2024\"\nversion = \"0.1.0\"\nversion_note = \"x\"\n\n\
                    [workspace.dependencies]\nserde = { version = \"1\" }\n";

        let (line, version) = field(toml, "version").unwrap();
        assert_eq!((line, version), (5, "0.1.0"));
        assert_eq!(
            set_line(toml, line, "version = \"0.2.0\""),
            toml.replace("version = \"0.1.0\"", "version = \"0.2.0\"")
        );
        assert!(field("[package]\nversion = \"1.0.0\"\n", "version").is_none());
    }

    #[test]
    fn the_real_cargo_toml_has_what_a_release_needs() {
        let toml = include_str!("../../../Cargo.toml");
        Version::parse(field(toml, "version").unwrap().1).unwrap();
        let repository = field(toml, "repository").unwrap().1;
        assert!(
            repository.starts_with("https://github.com/"),
            "{repository}"
        );
    }

    #[test]
    fn a_release_needs_changes_beyond_the_last_ones_bookkeeping() {
        let subjects = "Price foils on their own\nPoint the Homebrew formula at v0.2.0\n\
                        Release v0.2.0\nRead the badges again after sign-in";
        assert_eq!(
            worth_noting(subjects),
            [
                "Price foils on their own",
                "Read the badges again after sign-in"
            ]
        );
        let bookkeeping = "Point the Homebrew formula at v0.2.0\nRelease v0.2.0";
        assert!(worth_noting(bookkeeping).is_empty(), "nothing to release");
    }

    #[test]
    fn the_version_is_a_bump_or_given() {
        let args = |list: &[&str]| list.iter().map(|&a| a.to_owned()).collect::<Vec<_>>();

        let (wanted, yes) = arguments(&args(&["minor", "--yes"])).unwrap();
        assert!(matches!(wanted, Wanted::Bump(Bump::Minor)) && yes);
        let (wanted, yes) = arguments(&args(&["v0.3.0-rc.1"])).unwrap();
        assert!(matches!(wanted, Wanted::Exactly(v) if v.to_string() == "0.3.0-rc.1") && !yes);

        for bad in [
            &[][..],
            &["0.3"],
            &["minor", "major"],
            &["minor", "--force"],
        ] {
            assert!(arguments(&args(bad)).is_err(), "{bad:?}");
        }
    }
}
