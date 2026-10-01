//! Project tooling, run as `cargo xtask <task>` (the alias is in
//! `.cargo/config.toml`):
//!
//! - `setup`: gets a new clone ready. It installs the tools some checks need
//!   (see `TOOLS`), at the versions CI uses, and turns on the git hook.
//! - `ci`: everything CI checks, stopping at the first failure: the four
//!   groups below, which CI (`.github/workflows/tests.yml`) runs as jobs of
//!   their own.
//!   - `lint`: formatting, spelling, clippy, and dead code across crates;
//!   - `test`: the tests (on every OS);
//!   - `docs`: the docs, and the README's screenshots;
//!   - `deps`: unused dependencies, and the dependencies themselves.
//! - `pre-commit`: what the git hook runs before every commit (formatting,
//!   lints, tests).
//! - `fix`: formats, and applies clippy's suggestions.
//! - `hooks`: makes git run this project's hooks, in `.githooks/`.
//! - `tools [group]`: the tools a group's checks need (all of them, by
//!   default), as `crate@version`, so CI installs the versions `setup` does.
//! - `dead-code`: fails on a public function only tests use, or nothing
//!   does, which the compiler can't see. See `dead_code.rs`.
//! - `screenshots`: redraws the README's images of the TUI, in `docs/images/`.
//!   With `--check` (one of the `ci` checks), fails if they're out of date.
//! - `protect`: puts the rules for `main` and for tags (in `.github/rulesets/`)
//!   on GitHub.
//! - `release <patch | minor | major | X.Y.Z>`: publishes a release, from the
//!   version bump to Homebrew. See `release/mod.rs`.
//! - `release-notes <tag>`: a release's notes, for the release workflow.
//! - `homebrew <tag>`: points the Homebrew formula at a published release,
//!   as the release command does.
//!
//! Plain Rust, so it runs the same on Windows, macOS and Linux. A check that
//! needs a tool that isn't installed is skipped, with a hint, except in CI.

#![expect(
    clippy::print_stdout,
    clippy::print_stderr,
    reason = "a command-line tool reports on the terminal"
)]

mod dead_code;
mod protect;
mod release;
mod screenshots;

use std::{
    path::{Path, PathBuf},
    process::{Command, ExitCode},
    time::Instant,
};

/// The cargo that built this tool, so the checks use the same toolchain.
const CARGO: &str = env!("CARGO");

const USAGE: &str = "usage: cargo xtask <setup | ci | lint | test | docs | deps | pre-commit | fix | \
                     hooks | tools [group] | dead-code | screenshots [--check] | protect | \
                     release <patch | minor | major | X.Y.Z> | release-notes <tag> | homebrew <tag>>";

/// A program a check runs that doesn't come with Rust.
struct Tool {
    /// The crate `cargo install` builds it from.
    krate: &'static str,
    /// The version `cargo xtask setup` installs, and CI too.
    version: &'static str,
    run: Run,
}

/// How a tool is started.
enum Run {
    /// As `cargo <subcommand>`.
    Cargo(&'static str),
    /// As a program of its own.
    Program(&'static str),
}

const CARGO_MACHETE: Tool = Tool {
    krate: "cargo-machete",
    version: "0.9.2",
    run: Run::Cargo("machete"),
};

const CARGO_DENY: Tool = Tool {
    krate: "cargo-deny",
    version: "0.20.2",
    run: Run::Cargo("deny"),
};

const TYPOS_CLI: Tool = Tool {
    krate: "typos-cli",
    version: "1.50.3",
    run: Run::Program("typos"),
};

/// Every tool the checks need beyond Rust, with the one version of each that
/// they're checked with. `cargo xtask setup` installs these, and CI installs
/// the same ones from `cargo xtask tools`, so a check can't pass here and
/// fail there because the tool was different. Updating one is a change here.
const TOOLS: [&Tool; 3] = [&CARGO_MACHETE, &CARGO_DENY, &TYPOS_CLI];

struct Step {
    name: &'static str,
    /// What runs it: cargo, or one of `TOOLS`.
    tool: Option<&'static Tool>,
    /// The arguments for cargo, or for the tool.
    args: &'static [&'static str],
    /// Environment for the step's own process.
    env: &'static [(&'static str, &'static str)],
}

const FMT: Step = Step {
    name: "formatting",
    tool: None,
    args: &["fmt", "--all", "--check"],
    env: &[],
};

/// Spelling, in code, comments and docs. What it accepts is in `typos.toml`.
const TYPOS: Step = Step {
    name: "spelling",
    tool: Some(&TYPOS_CLI),
    args: &[],
    env: &[],
};

const CLIPPY: Step = Step {
    name: "lints",
    tool: None,
    args: &[
        "clippy",
        "--workspace",
        "--all-targets",
        "--locked",
        "--",
        "-D",
        "warnings",
    ],
    env: &[],
};

const TEST: Step = Step {
    name: "tests",
    tool: None,
    args: &["test", "--workspace", "--locked"],
    env: &[],
};

/// No public function is used only by tests, or by nothing: dead code the
/// compiler can't see across crates.
const DEAD_CODE: Step = Step {
    name: "dead code",
    tool: None,
    args: &["xtask", "dead-code"],
    env: &[],
};

/// The README's images of the TUI are what it draws now. The previews that
/// draw them run on a fixed clock, so this changes only when the UI does.
const SCREENSHOTS: Step = Step {
    name: "README screenshots",
    tool: None,
    args: &["xtask", "screenshots", "--check"],
    env: &[],
};

const DOC: Step = Step {
    name: "docs",
    tool: None,
    args: &["doc", "--workspace", "--no-deps", "--locked"],
    env: &[("RUSTDOCFLAGS", "-D warnings")],
};

const MACHETE: Step = Step {
    name: "unused dependencies",
    tool: Some(&CARGO_MACHETE),
    args: &[],
    env: &[],
};

/// Security advisories, licences, duplicate versions and where crates come
/// from, for every dependency. The rules are in `deny.toml`. It leaves out
/// the dependency graphs, so the warnings about duplicates don't bury an
/// error; `cargo tree -i <crate>` shows what brings a crate in.
const DENY: Step = Step {
    name: "dependencies",
    tool: Some(&CARGO_DENY),
    args: &["--workspace", "--locked", "check", "--hide-inclusion-graph"],
    env: &[],
};

/// The groups of checks CI runs as jobs of their own, in tests.yml: the
/// tests on every OS, and the others once.
const LINT: [Step; 4] = [FMT, TYPOS, CLIPPY, DEAD_CODE];
const TESTS: [Step; 1] = [TEST];
const DOCS: [Step; 2] = [DOC, SCREENSHOTS];
const DEPS: [Step; 2] = [MACHETE, DENY];

/// Everything CI checks, one group after another.
const CI: [Step; 9] = [
    FMT,
    TYPOS,
    CLIPPY,
    DEAD_CODE,
    TEST,
    DOC,
    SCREENSHOTS,
    MACHETE,
    DENY,
];

/// A group of checks by name, as `cargo xtask <group>` and
/// `cargo xtask tools <group>` take it.
fn group(name: &str) -> Option<&'static [Step]> {
    Some(match name {
        "lint" => &LINT,
        "test" => &TESTS,
        "docs" => &DOCS,
        "deps" => &DEPS,
        "ci" => &CI,
        _ => return None,
    })
}

const FIX: [Step; 3] = [
    Step {
        name: "format",
        tool: None,
        args: &["fmt", "--all"],
        env: &[],
    },
    Step {
        name: "clippy fixes",
        tool: None,
        args: &[
            "clippy",
            "--workspace",
            "--all-targets",
            "--fix",
            "--allow-dirty",
            "--allow-staged",
        ],
        env: &[],
    },
    // Clippy's fixes can leave code unformatted.
    Step {
        name: "format again",
        tool: None,
        args: &["fmt", "--all"],
        env: &[],
    },
];

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let ok = match args.first().map(String::as_str) {
        Some("setup") => setup(),
        Some(name @ ("ci" | "lint" | "test" | "docs" | "deps")) => group(name).is_some_and(run),
        Some("pre-commit") => run(&[FMT, CLIPPY, TEST]),
        Some("fix") => run(&FIX),
        Some("hooks") => hooks(),
        Some("tools") => tools(args.get(1).map_or("ci", String::as_str)),
        Some("dead-code") => task(dead_code::run, &args[1..]),
        Some("screenshots") => task(screenshots::run, &args[1..]),
        Some("protect") => task(protect::run, &args[1..]),
        Some("release") => task(release::run, &args[1..]),
        Some("release-notes") => task(release::print_notes, &args[1..]),
        Some("homebrew") => task(release::point_homebrew, &args[1..]),
        _ => {
            eprintln!("{USAGE}");
            false
        }
    };
    if ok {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

/// Runs the steps in order, from the workspace's root, and stops at the first
/// failure.
fn run(steps: &[Step]) -> bool {
    // The tools check the folder they're started in, so a run from a
    // subfolder would check only that.
    let root = match workspace_root() {
        Ok(root) => root,
        Err(e) => {
            eprintln!("✗ {e}");
            return false;
        }
    };
    let started = Instant::now();
    let mut skipped = Vec::new();
    for step in steps {
        let (mut command, program) = match step.tool {
            None => (process(CARGO), "cargo".to_owned()),
            Some(tool) => match tool.installed() {
                Some(version) => {
                    if version != tool.version {
                        println!(
                            "! {}: {} {version} is installed, but CI uses {} \
                             (`cargo xtask setup` installs it)",
                            step.name, tool.krate, tool.version
                        );
                    }
                    (tool.command(), tool.typed())
                }
                // CI installs every tool, so one missing there is a mistake,
                // and skipping would pass a check that never ran.
                None if in_ci() => {
                    eprintln!("\n✗ {}: {} isn't installed", step.name, tool.krate);
                    return false;
                }
                None => {
                    println!(
                        "- {}: skipped, {} isn't installed (`cargo xtask setup` installs it)",
                        step.name, tool.krate
                    );
                    skipped.push(step.name);
                    continue;
                }
            },
        };
        let words: Vec<&str> = std::iter::once(program.as_str())
            .chain(step.args.iter().copied())
            .collect();
        println!("> {}: {}", step.name, words.join(" "));
        let ok = command
            .args(step.args)
            .envs(step.env.iter().copied())
            .current_dir(&root)
            .status()
            .is_ok_and(|s| s.success());
        if !ok {
            eprintln!("\n✗ {} failed", step.name);
            return false;
        }
    }
    let ran = steps.len() - skipped.len();
    let checks = if steps.len() == 1 { "check" } else { "checks" };
    println!(
        "\n✓ {ran} of {} {checks} passed in {}s",
        steps.len(),
        started.elapsed().as_secs()
    );
    true
}

impl Tool {
    /// How it's typed: `cargo deny`, or `typos`.
    fn typed(&self) -> String {
        match self.run {
            Run::Cargo(subcommand) => format!("cargo {subcommand}"),
            Run::Program(program) => program.to_owned(),
        }
    }

    /// A command that starts it, for the arguments to follow.
    fn command(&self) -> Command {
        match self.run {
            Run::Cargo(subcommand) => {
                let mut command = process(CARGO);
                command.arg(subcommand);
                command
            }
            Run::Program(program) => process(program),
        }
    }

    /// The version that runs, or `None` if it isn't installed.
    fn installed(&self) -> Option<String> {
        let out = self.command().arg("--version").output().ok()?;
        let stdout = String::from_utf8_lossy(&out.stdout);
        // `cargo-deny 0.20.2`, `typos-cli 1.50.3`, or just `0.9.2`.
        let version = stdout.lines().next()?.split_whitespace().last()?;
        out.status.success().then(|| version.to_owned())
    }
}

/// A command for `program`, without what `cargo run` set for xtask itself.
fn process(program: &str) -> Command {
    let mut command = Command::new(program);
    // Passed on, this makes cargo-machete think it wasn't started as
    // `cargo machete`, and look for a folder called "machete".
    command.env_remove("CARGO_PKG_NAME");
    command
}

/// Whether this is CI, where every check has to run. GitHub Actions, like
/// most CI services, sets `CI=true`.
#[expect(
    clippy::disallowed_methods,
    reason = "a tool cargo runs, not the app, whose settings are read in one place"
)]
fn in_ci() -> bool {
    std::env::var_os("CI").is_some_and(|value| value == "true")
}

/// Gets a new clone ready: installs each of `TOOLS` that's missing or at
/// another version, then turns on the git hook.
fn setup() -> bool {
    let mut ok = true;
    for tool in TOOLS {
        ok &= install(tool);
    }
    // The hook doesn't need the tools, so it goes on either way.
    hooks() && ok
}

/// Installs `tool` at its version with `cargo install`, unless that's the
/// version installed already.
fn install(tool: &Tool) -> bool {
    let (krate, version) = (tool.krate, tool.version);
    let found = match tool.installed() {
        Some(found) if found == version => {
            println!("✓ {krate} {version}: already installed");
            return true;
        }
        Some(found) => format!("{found} is installed"),
        None => "not installed yet".to_owned(),
    };
    println!("> {krate} {version}: installing ({found})");
    let built = process(CARGO)
        .args(["install", "--locked", &format!("{krate}@{version}")])
        // rustup tells cargo the compiler came from rust-toolchain.toml, and
        // cargo would warn that it isn't your default one. Building the tools
        // with the project's compiler is on purpose.
        .env_remove("RUSTUP_TOOLCHAIN_SOURCE")
        .status()
        .is_ok_and(|s| s.success());
    if !built {
        eprintln!("✗ {krate} {version}: couldn't install it");
        return false;
    }
    // Another copy that comes first on PATH would still be the one that runs.
    match tool.installed() {
        Some(found) if found == version => {
            println!("✓ {krate} {version}: installed");
            true
        }
        Some(found) => {
            eprintln!(
                "✗ {krate} {version}: installed, but {found} runs instead. \
                 Another copy comes before cargo's on your PATH."
            );
            false
        }
        None => {
            eprintln!(
                "✗ {krate} {version}: installed, but it doesn't run. \
                 Is cargo's bin folder on your PATH?"
            );
            false
        }
    }
}

/// Points git at the project's hooks. Git never runs a repository's own hooks
/// until the clone opts in, so this is the opt-in.
fn hooks() -> bool {
    let ok = Command::new("git")
        .args(["config", "core.hooksPath", ".githooks"])
        .status()
        .is_ok_and(|s| s.success());
    if !ok {
        eprintln!("✗ couldn't set core.hooksPath (is git installed, and is this a clone?)");
        return false;
    }
    println!(
        "✓ git runs .githooks/pre-commit before every commit: formatting, lints and tests.\n  \
         Skip it once with `git commit --no-verify`."
    );
    true
}

/// Prints the tools a group's checks need, as `crate@version,…`: what CI
/// installs for that job.
fn tools(name: &str) -> bool {
    let Some(steps) = group(name) else {
        eprintln!("✗ no group called {name:?}: lint, test, docs, deps or ci");
        return false;
    };
    let tools: Vec<String> = steps
        .iter()
        .filter_map(|step| step.tool)
        .map(|tool| format!("{}@{}", tool.krate, tool.version))
        .collect();
    println!("{}", tools.join(","));
    true
}

/// Runs a task on the checkout, and reports what went wrong.
fn task(run: fn(&Path, &[String]) -> Result<(), String>, args: &[String]) -> bool {
    match workspace_root().and_then(|root| run(&root, args)) {
        Ok(()) => true,
        Err(e) => {
            eprintln!("\n✗ {e}");
            false
        }
    }
}

/// The checkout, as cargo gives it when it runs xtask: read when run rather
/// than built in, so a moved checkout still finds itself.
#[expect(
    clippy::disallowed_methods,
    reason = "a tool cargo runs, not the app, whose settings are read in one place"
)]
fn workspace_root() -> Result<PathBuf, String> {
    let xtask = std::env::var_os("CARGO_MANIFEST_DIR").ok_or("run this through `cargo xtask`")?;
    Path::new(&xtask)
        .parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| "no workspace root above xtask".into())
}
