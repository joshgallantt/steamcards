//! `dead-code`: dead code the compiler can't see. Rust judges dead code a
//! crate at a time, and counts whatever a crate exports as used. No crate
//! here is published, so a public function that nothing in the workspace
//! calls is dead, and so is one that only tests call: that's test support,
//! and belongs behind the `test-support` feature, or in the test.
//!
//! It goes by name: each `pub fn` in a library's own code, and whether any
//! other code names it. Tests are what's in `tests/` folders, `test_support`
//! modules, a module declared `#[cfg(test)]`, and a file's own tests, which
//! come last in it. A function behind `cfg(test)` or the `test-support`
//! feature is test support already. Binaries (`app/`, `xtask/`) export
//! nothing, so the compiler sees all of theirs. A name another function
//! shares counts as used, so this can miss dead code, but never calls used
//! code dead.

use std::{collections::HashMap, path::Path, process::Command};

/// Fails, naming each one, if a library's public function is used only by
/// tests, or not at all.
pub(crate) fn run(root: &Path, args: &[String]) -> Result<(), String> {
    if !args.is_empty() {
        return Err("usage: cargo xtask dead-code".into());
    }
    let listed = Command::new("git")
        .args(["ls-files", "-z", "*.rs"])
        .current_dir(root)
        .output()
        .map_err(|e| format!("couldn't run git: {e}"))?;
    if !listed.status.success() {
        return Err("git couldn't list the Rust files".into());
    }
    let mut files = Vec::new();
    for path in String::from_utf8_lossy(&listed.stdout).split('\0') {
        if path.is_empty() {
            continue;
        }
        let text = std::fs::read_to_string(root.join(path))
            .map_err(|e| format!("couldn't read {path}: {e}"))?;
        files.push((path.to_owned(), text));
    }
    let (checked, dead) = find(&files);
    if dead.is_empty() {
        println!("{checked} public functions, each used by more than tests");
        return Ok(());
    }
    for found in &dead {
        eprintln!("{found}");
    }
    Err(format!(
        "{} of {checked} public functions are used only by tests, or not at all: \
         move test support behind `test-support`, and remove what nothing uses",
        dead.len()
    ))
}

/// A file, split into its code and its tests.
struct Source<'a> {
    path: &'a str,
    code: &'a str,
    tests: &'a str,
}

/// How many public functions there are, and what's said of each that only
/// tests use, or nothing does.
fn find(files: &[(String, String)]) -> (usize, Vec<String>) {
    let test_modules = test_modules(files);
    let sources: Vec<Source<'_>> = files
        .iter()
        .map(|(path, text)| {
            if is_test_file(path) || test_modules.iter().any(|m| m == path) {
                Source {
                    path,
                    code: "",
                    tests: text,
                }
            } else {
                let (code, tests) = split_tests(text);
                Source { path, code, tests }
            }
        })
        .collect();

    let mut in_code: HashMap<&str, usize> = HashMap::new();
    let mut in_tests: HashMap<&str, usize> = HashMap::new();
    for source in &sources {
        for name in names(source.code) {
            *in_code.entry(name).or_default() += 1;
        }
        for name in names(source.tests) {
            *in_tests.entry(name).or_default() += 1;
        }
    }

    let defined: Vec<(&str, &str)> = sources
        .iter()
        .filter(|s| is_library(s.path))
        .flat_map(|s| {
            public_fns(s.code)
                .into_iter()
                .map(move |name| (s.path, name))
        })
        .collect();
    let mut times_defined: HashMap<&str, usize> = HashMap::new();
    for &(_, name) in &defined {
        *times_defined.entry(name).or_default() += 1;
    }

    let mut dead = Vec::new();
    for &(path, name) in &defined {
        // Each definition names it once.
        let used = in_code
            .get(name)
            .copied()
            .unwrap_or_default()
            .saturating_sub(times_defined[name]);
        if used > 0 {
            continue;
        }
        let by_tests = in_tests.get(name).copied().unwrap_or_default() > 0;
        dead.push(if by_tests {
            format!("{path}: `{name}` is used only by tests")
        } else {
            format!("{path}: `{name}` is used nowhere")
        });
    }
    (defined.len(), dead)
}

/// Whether the whole file is tests, or test support, by where it is.
fn is_test_file(path: &str) -> bool {
    let path = format!("/{path}");
    path.contains("/tests/")
        || path.contains("/test_support/")
        || path.ends_with("/test_support.rs")
}

/// Whether it's a library's code, whose exports the compiler can't check:
/// not a binary's.
fn is_library(path: &str) -> bool {
    !(path.starts_with("app/") || path.starts_with("xtask/"))
}

/// The files of modules declared `#[cfg(test)]`, like the dashboard's
/// previews: `#[cfg(test)] mod preview;` in `app/mod.rs` is
/// `app/preview.rs`, or `app/preview/mod.rs`.
fn test_modules(files: &[(String, String)]) -> Vec<String> {
    let mut modules = Vec::new();
    for (path, text) in files {
        let Some((dir, file)) = path.rsplit_once('/') else {
            continue;
        };
        // A file declares its modules beside it if it's a crate's or a
        // folder's root, and in a folder named for it otherwise.
        let dir = match file {
            "lib.rs" | "main.rs" | "mod.rs" => dir.to_owned(),
            other => format!("{dir}/{}", other.trim_end_matches(".rs")),
        };
        let mut lines = text.lines().map(str::trim);
        while let Some(line) = lines.next() {
            if line != "#[cfg(test)]" {
                continue;
            }
            let Some(next) = lines.next() else {
                break;
            };
            let declared = next
                .trim_start_matches("pub(crate) ")
                .strip_prefix("mod ")
                .and_then(|rest| rest.strip_suffix(';'));
            if let Some(name) = declared {
                modules.push(format!("{dir}/{name}.rs"));
                modules.push(format!("{dir}/{name}/mod.rs"));
            }
        }
    }
    modules
}

/// A file's code, and the tests that come last in it: from `#[cfg(test)]`
/// on a module with its body here.
fn split_tests(text: &str) -> (&str, &str) {
    let mut offset = 0;
    let mut lines = text.split_inclusive('\n').peekable();
    while let Some(line) = lines.next() {
        if line.trim() == "#[cfg(test)]"
            && lines.peek().is_some_and(|next| {
                let next = next.trim();
                next.starts_with("mod ") && next.ends_with('{')
            })
        {
            return text.split_at(offset);
        }
        offset += line.len();
    }
    (text, "")
}

/// The public functions declared in `code`, but for any behind `cfg(test)`
/// or the `test-support` feature: those are tests' already.
fn public_fns(code: &str) -> Vec<&str> {
    let lines: Vec<&str> = code.lines().map(str::trim).collect();
    let mut found = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        let Some(rest) = ["pub fn ", "pub async fn ", "pub const fn "]
            .iter()
            .find_map(|start| line.strip_prefix(start))
        else {
            continue;
        };
        let rest = rest.trim_start_matches("r#");
        let end = rest
            .find(|c: char| !(c.is_alphanumeric() || c == '_'))
            .unwrap_or(rest.len());
        let name = &rest[..end];
        if name.is_empty() || name.starts_with(|c: char| c.is_ascii_digit()) {
            continue;
        }
        // Its attributes and doc comment, above it.
        let gated = lines[..i]
            .iter()
            .rev()
            .take_while(|l| l.starts_with("#[") || l.starts_with("///"))
            .any(|l| is_test_cfg(l));
        if !gated {
            found.push(name);
        }
    }
    found
}

/// Whether an attribute keeps what follows to tests: `#[cfg(test)]`,
/// `#[cfg(feature = "test-support")]`, or both. Not `cfg(not(test))`.
fn is_test_cfg(attribute: &str) -> bool {
    attribute.starts_with("#[cfg(")
        && !attribute.contains("not(")
        && attribute
            .split(|c: char| !(c.is_alphanumeric() || c == '_'))
            .any(|word| word == "test")
}

/// Every name in `text`, once each time it appears, but for those in
/// comments: a doc comment that mentions a function doesn't use it.
fn names(text: &str) -> impl Iterator<Item = &str> {
    text.lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .flat_map(|line| {
            line.split(|c: char| !(c.is_alphanumeric() || c == '_'))
                .filter(|word| word.starts_with(|c: char| c.is_alphabetic() || c == '_'))
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn workspace(files: &[(&str, &str)]) -> Vec<(String, String)> {
        files
            .iter()
            .map(|&(path, text)| (path.to_owned(), text.to_owned()))
            .collect()
    }

    const LIBRARY: &str = "\
pub fn read_library() {}

pub fn asked_only_in_tests() {}

pub fn never_asked() {}

/// For tests that can't wait.
#[cfg(feature = \"test-support\")]
pub fn with_patience() {}

pub(crate) fn inside() {}

#[cfg(test)]
mod tests {
    #[test]
    fn reads() {
        super::asked_only_in_tests();
    }
}
";

    #[test]
    fn a_function_only_tests_use_is_dead_and_so_is_one_nothing_uses() {
        let files = workspace(&[
            ("component/library/domain/src/lib.rs", LIBRARY),
            (
                "app/src/main.rs",
                "fn main() { steam_library::read_library(); }",
            ),
        ]);

        let (checked, dead) = find(&files);

        assert_eq!(checked, 3, "pub(crate) and test support aren't checked");
        assert_eq!(
            dead,
            [
                "component/library/domain/src/lib.rs: `asked_only_in_tests` is used only by tests",
                "component/library/domain/src/lib.rs: `never_asked` is used nowhere",
            ]
        );
    }

    #[test]
    fn test_files_and_test_modules_are_tests() {
        let files = workspace(&[
            ("component/card/domain/src/lib.rs", "pub fn look() {}\n"),
            (
                "component/card/domain/tests/looking.rs",
                "fn looks() { card::look(); }",
            ),
            (
                "ui/terminal-ui/src/app/mod.rs",
                "#[cfg(test)]\nmod preview;\n",
            ),
            (
                "ui/terminal-ui/src/app/preview.rs",
                "fn draws() { card::look(); }",
            ),
            (
                "library/steam-api/src/test_support.rs",
                "pub fn stand_in() { card::look(); }",
            ),
        ]);

        let (_, dead) = find(&files);

        assert_eq!(
            dead,
            ["component/card/domain/src/lib.rs: `look` is used only by tests"],
            "the test support's own functions are tests' too"
        );
    }

    #[test]
    fn a_doc_comment_naming_a_function_doesnt_use_it() {
        let files = workspace(&[(
            "library/debug-log/src/lib.rs",
            "/// Calls `held` first.\npub fn held() {}\n",
        )]);

        assert_eq!(
            find(&files).1,
            ["library/debug-log/src/lib.rs: `held` is used nowhere"]
        );
    }
}
