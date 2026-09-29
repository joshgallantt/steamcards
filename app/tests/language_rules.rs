//! Rust features this codebase doesn't use, checked. Each is a way to reach
//! across a boundary without the boundary showing it. See
//! docs/architecture.md, "Rust features this codebase doesn't use".
//!
//! Clippy covers glob imports, printing and reading the environment (see the
//! workspace lints and clippy.toml). These are the rules it can't express.

use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};

/// Every crate in the workspace: its directory and its source files.
fn crates() -> Vec<(PathBuf, Vec<(PathBuf, String)>)> {
    #[expect(
        clippy::disallowed_methods,
        reason = "cargo tells a test where it is through the environment"
    )]
    let app = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("run under cargo"));
    let root = app.parent().unwrap().to_path_buf();

    let mut dirs = vec![app.clone(), root.join("xtask")];
    for group in ["component", "library", "ui"] {
        collect_crates(&root.join(group), &mut dirs);
    }
    dirs.into_iter()
        .map(|dir| {
            let mut files = Vec::new();
            for sub in ["src", "tests"] {
                collect_rs(&dir.join(sub), &mut files);
            }
            let files = files
                .into_iter()
                .map(|f| {
                    let text = fs::read_to_string(&f).unwrap();
                    (f.strip_prefix(&root).unwrap().to_path_buf(), text)
                })
                .collect();
            (dir, files)
        })
        .collect()
}

fn collect_crates(dir: &Path, out: &mut Vec<PathBuf>) {
    if dir.join("Cargo.toml").exists() {
        out.push(dir.to_path_buf());
        return;
    }
    for e in fs::read_dir(dir).into_iter().flatten().flatten() {
        if e.path().is_dir() {
            collect_crates(&e.path(), out);
        }
    }
}

fn collect_rs(dir: &Path, out: &mut Vec<PathBuf>) {
    for e in fs::read_dir(dir).into_iter().flatten().flatten() {
        let p = e.path();
        if p.is_dir() {
            collect_rs(&p, out);
        } else if p.extension().is_some_and(|x| x == "rs") {
            out.push(p);
        }
    }
}

/// Lines of code, with `//` comments removed.
fn code_lines(text: &str) -> impl Iterator<Item = (usize, &str)> {
    text.lines()
        .enumerate()
        .map(|(i, l)| (i + 1, l.split("//").next().unwrap().trim()))
}

/// The item keyword's name: `pub(crate) struct Foo<T> {` → `Foo` for `struct`.
fn declared(line: &str, keyword: &str) -> Option<String> {
    let rest = line
        .trim_start_matches("pub(crate) ")
        .trim_start_matches("pub(super) ")
        .trim_start_matches("pub ")
        .strip_prefix(keyword)?
        .strip_prefix(' ')?;
    let name: String = rest
        .chars()
        .take_while(|c| c.is_alphanumeric() || *c == '_')
        .collect();
    (!name.is_empty()).then_some(name)
}

/// Last path segment without generics or references: `&'a mut foo::Bar<T>` → `Bar`.
fn last_segment(path: &str) -> String {
    let path = path.trim().trim_start_matches('&');
    let path = path.split_whitespace().last().unwrap_or("");
    let path = path.split('<').next().unwrap();
    path.rsplit("::").next().unwrap().to_owned()
}

/// `(trait, self type)` for every `impl Trait for Type` line.
fn trait_impls(line: &str) -> Option<(String, String)> {
    let rest = line.strip_prefix("impl")?;
    let rest = if rest.starts_with('<') {
        // Skip the impl's own generics, which may nest.
        let mut depth = 0;
        let end = rest.char_indices().find_map(|(i, c)| {
            match c {
                '<' => depth += 1,
                '>' => depth -= 1,
                _ => {}
            }
            (depth == 0).then_some(i + 1)
        })?;
        &rest[end..]
    } else {
        rest
    };
    let (tr, ty) = rest.split_once(" for ")?;
    let ty = ty.split('{').next().unwrap();
    let ty = ty.split(" where ").next().unwrap();
    Some((last_segment(tr), last_segment(ty)))
}

#[test]
fn no_global_state() {
    // A static is a service locator: reachable from anywhere, handed in by
    // nobody. State belongs to a value the composition root builds.
    let mut wrong = Vec::new();
    for (_, files) in crates() {
        for (path, text) in &files {
            for (n, line) in code_lines(text) {
                let is_static = ["static ", "pub static ", "pub(crate) static "]
                    .iter()
                    .any(|p| line.starts_with(p));
                let is_macro = ["thread_local", "lazy_static"]
                    .iter()
                    .any(|m| line.contains(&format!("{m}!")));
                if is_static || is_macro {
                    wrong.push(format!("{}:{n}: {line}", path.display()));
                }
            }
        }
    }
    assert!(wrong.is_empty(), "global state:\n{}", wrong.join("\n"));
}

#[test]
fn no_extension_traits() {
    // Rust's version of a Swift extension: a trait declared only to bolt
    // methods onto a type its crate doesn't own. Behaviour then lives away
    // from its type and appears wherever the trait is imported. A crate may
    // implement its own traits for its own types, and other crates' traits
    // (the contracts) for its own types — nothing else.
    let mut wrong = Vec::new();
    for (_, files) in crates() {
        let mut traits = BTreeSet::new();
        let mut types = BTreeSet::new();
        for (_, text) in &files {
            for (_, line) in code_lines(text) {
                traits.extend(declared(line, "trait"));
                for kw in ["struct", "enum", "type"] {
                    types.extend(declared(line, kw));
                }
            }
        }
        for (path, text) in &files {
            for (n, line) in code_lines(text) {
                let Some((tr, ty)) = trait_impls(line) else {
                    continue;
                };
                if traits.contains(&tr) && !types.contains(&ty) {
                    wrong.push(format!("{}:{n}: {tr} for {ty}", path.display()));
                }
            }
        }
    }
    assert!(wrong.is_empty(), "extension traits:\n{}", wrong.join("\n"));
}

#[test]
fn no_inheritance_through_deref() {
    // `Deref` to an inner type makes one type pretend to be another: every
    // inner method appears on the outer one, unasked for. Hold the inner
    // value and forward what's needed.
    let mut wrong = Vec::new();
    for (_, files) in crates() {
        for (path, text) in &files {
            for (n, line) in code_lines(text) {
                if let Some((tr, _)) = trait_impls(line)
                    && (tr == "Deref" || tr == "DerefMut")
                {
                    wrong.push(format!("{}:{n}: {line}", path.display()));
                }
            }
        }
    }
    assert!(
        wrong.is_empty(),
        "Deref used as inheritance:\n{}",
        wrong.join("\n")
    );
}

#[test]
fn the_rules_catch_what_they_are_for() {
    assert_eq!(
        declared("pub(crate) struct Foo<T> {", "struct").as_deref(),
        Some("Foo")
    );
    assert_eq!(
        declared("pub trait StrExt: Sized {", "trait").as_deref(),
        Some("StrExt")
    );
    assert_eq!(
        trait_impls("impl<T: Clone> StrExt for Vec<T> where T: Send {"),
        Some(("StrExt".into(), "Vec".into()))
    );
    assert_eq!(
        trait_impls("impl farming::CardsRepository for SteamCardsRepository {"),
        Some(("CardsRepository".into(), "SteamCardsRepository".into()))
    );
    assert_eq!(
        trait_impls("impl<'a> Deref for &'a Wrapper {"),
        Some(("Deref".into(), "Wrapper".into()))
    );
    assert_eq!(trait_impls("impl Foo {"), None);
}
