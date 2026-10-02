//! The Homebrew formula: pointing it at a release's macOS archives. The
//! release command does this once the release is published, and pushes it to
//! `main`; `.github/workflows/homebrew.yml` then installs it and checks it.

use std::{collections::HashMap, path::Path};

use super::version::Version;

pub(super) const FORMULA: &str = "Formula/steamcards.rb";

/// What pointing the formula at a release did.
pub(super) enum Pointed {
    Changed,
    /// It pointed there already.
    Unchanged,
    /// It points at this newer version, so it was left alone: finishing an
    /// old release never takes Homebrew back.
    Newer(String),
}

/// Points the formula at release `tag`, using the checksums in its
/// SHA256SUMS.
pub(super) fn point(root: &Path, tag: &str) -> Result<Pointed, String> {
    let version = tag
        .strip_prefix('v')
        .ok_or_else(|| format!("{tag} isn't a release tag, like v0.2.0"))?;
    let path = root.join(FORMULA);
    let formula = super::read(&path)?;
    if let Some(current) = formula_version(&formula)
        && Version::parse(current)? > Version::parse(version)?
    {
        return Ok(Pointed::Newer(current.to_owned()));
    }
    let sums = super::output(
        root,
        "gh",
        &[
            "release",
            "download",
            tag,
            "--pattern",
            "SHA256SUMS",
            "--output",
            "-",
        ],
    )?;
    let pointed = point_formula(&formula, version, &parse_sums(&sums))?;
    if pointed == formula {
        return Ok(Pointed::Unchanged);
    }
    super::write(&path, &pointed)?;
    Ok(Pointed::Changed)
}

/// The version the formula installs.
fn formula_version(formula: &str) -> Option<&str> {
    formula.lines().find_map(|line| {
        line.trim_start()
            .strip_prefix("version \"")?
            .strip_suffix('"')
    })
}

/// A SHA256SUMS file: each archive's name, and its checksum.
fn parse_sums(text: &str) -> HashMap<String, String> {
    text.lines()
        .filter_map(|line| {
            let (sum, name) = line.split_once(char::is_whitespace)?;
            let name = name.trim().trim_start_matches('*');
            Some((name.to_owned(), sum.to_owned()))
        })
        .collect()
}

/// The formula pointed at release `version`: its version, each archive's URL,
/// and each archive's checksum. Each url line has to be followed by its
/// sha256 line.
fn point_formula(
    formula: &str,
    version: &str,
    sums: &HashMap<String, String>,
) -> Result<String, String> {
    let mut out = Vec::new();
    // The archive whose sha256 line comes next.
    let mut archive: Option<&str> = None;
    for line in formula.lines() {
        let code = line.trim_start();
        let indent = &line[..line.len() - code.len()];
        if code.starts_with("sha256 ") {
            let name = archive
                .take()
                .ok_or("the formula has a sha256 line that doesn't follow a url line")?;
            let sum = sums
                .get(name)
                .filter(|s| s.len() == 64 && s.chars().all(|c| c.is_ascii_hexdigit()))
                .ok_or_else(|| format!("the release's SHA256SUMS has no checksum for {name}"))?;
            out.push(format!("{indent}sha256 \"{sum}\""));
            continue;
        }
        if let Some(name) = archive {
            return Err(format!(
                "the formula's url line for {name} isn't followed by its sha256 line"
            ));
        }
        if code.starts_with("version \"") {
            out.push(format!("{indent}version \"{version}\""));
        } else if let Some(url) = code
            .strip_prefix("url \"")
            .and_then(|rest| rest.strip_suffix('"'))
        {
            let (release, name) = url
                .rsplit_once('/')
                .ok_or_else(|| format!("the formula's url {url} names no file"))?;
            let (downloads, _old_tag) = release
                .rsplit_once('/')
                .filter(|(downloads, _)| downloads.ends_with("/releases/download"))
                .ok_or_else(|| format!("the formula's url {url} isn't a release download"))?;
            out.push(format!("{indent}url \"{downloads}/v{version}/{name}\""));
            archive = Some(name);
        } else {
            out.push(line.to_owned());
        }
    }
    let mut text = out.join("\n");
    if formula.ends_with('\n') {
        text.push('\n');
    }
    Ok(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    const REAL: &str = include_str!("../../../Formula/steamcards.rb");

    fn sums(arm: char, intel: char) -> HashMap<String, String> {
        HashMap::from([
            (
                "steamcards-aarch64-apple-darwin.tar.gz".into(),
                arm.to_string().repeat(64),
            ),
            (
                "steamcards-x86_64-apple-darwin.tar.gz".into(),
                intel.to_string().repeat(64),
            ),
            (
                "steamcards-x86_64-unknown-linux-musl.tar.gz".into(),
                "c".repeat(64),
            ),
        ])
    }

    #[test]
    fn the_formulas_version_is_read() {
        assert_eq!(formula_version("  version \"1.2.3\"\n"), Some("1.2.3"));
        assert_eq!(formula_version("class X\nend\n"), None);
        // Whichever release the real one is on.
        Version::parse(formula_version(REAL).unwrap()).unwrap();
    }

    #[test]
    fn checksums_are_read_in_either_format() {
        let sums = parse_sums("abc  steamcards-a.tar.gz\ndef *steamcards-b.tar.gz\n\n");
        assert_eq!(sums["steamcards-a.tar.gz"], "abc");
        assert_eq!(sums["steamcards-b.tar.gz"], "def");
    }

    #[test]
    fn the_real_formula_is_pointed_at_a_release() {
        let pointed = point_formula(REAL, "0.2.0", &sums('a', 'b')).unwrap();

        assert!(pointed.contains("  version \"0.2.0\"\n"));
        let arm = "/releases/download/v0.2.0/steamcards-aarch64-apple-darwin.tar.gz\"\n";
        assert!(pointed.contains(&format!("{arm}    sha256 \"{}\"", "a".repeat(64))));
        let intel = "/releases/download/v0.2.0/steamcards-x86_64-apple-darwin.tar.gz\"\n";
        assert!(pointed.contains(&format!("{intel}    sha256 \"{}\"", "b".repeat(64))));
        assert_eq!(
            pointed.lines().count(),
            REAL.lines().count(),
            "nothing else changes"
        );

        let again = point_formula(&pointed, "0.2.0", &sums('a', 'b')).unwrap();
        assert_eq!(again, pointed, "pointing it twice changes nothing");
    }

    #[test]
    fn a_formula_that_cant_be_pointed_is_refused() {
        let mut missing = sums('a', 'b');
        missing.remove("steamcards-x86_64-apple-darwin.tar.gz");
        let err = point_formula(REAL, "0.2.0", &missing).unwrap_err();
        assert!(err.contains("x86_64-apple-darwin"), "{err}");

        let short = HashMap::from([(
            "steamcards-aarch64-apple-darwin.tar.gz".to_owned(),
            "abc".to_owned(),
        )]);
        let one = "  url \"https://github.com/o/r/releases/download/v1.0.0/steamcards-aarch64-apple-darwin.tar.gz\"\n  sha256 \"0\"\n";
        assert!(
            point_formula(one, "0.2.0", &short).is_err(),
            "not a checksum"
        );

        let unpaired = "  url \"https://github.com/o/r/releases/download/v1.0.0/steamcards-aarch64-apple-darwin.tar.gz\"\n  depends_on :macos\n";
        assert!(point_formula(unpaired, "0.2.0", &sums('a', 'b')).is_err());

        let elsewhere = "  url \"https://example.com/steamcards-aarch64-apple-darwin.tar.gz\"\n  sha256 \"0\"\n";
        assert!(point_formula(elsewhere, "0.2.0", &sums('a', 'b')).is_err());
    }
}
