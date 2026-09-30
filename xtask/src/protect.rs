//! `cargo xtask protect`: puts the repository's rules on GitHub.
//!
//! The rules are GitHub rulesets, kept in `.github/rulesets/`:
//! - `main.json`: changes reach `main` through pull requests, once the Tests
//!   workflow passes; only the repository's admin (the maintainer) can
//!   update `main`, by merging a pull request or pushing, which is how
//!   releases land; and `main` can't be force-pushed or deleted.
//! - `tags.json`: only the admin can create, move or delete a tag. Pushing a
//!   `v*` tag is what publishes a release, and creating a release on GitHub
//!   makes a tag, so only the admin can publish one.
//!
//! The admin can go around the rules when needed, but nobody else can.
//!
//! This creates each ruleset, or brings it back in line with its file. It
//! needs the GitHub CLI, signed in as the repository's admin, and GitHub only
//! enforces rules on a public repository, or on a private one with GitHub
//! Pro.

use std::{fs, path::Path, process::Command};

use serde_json::Value;

const RULESETS: [&str; 2] = [".github/rulesets/main.json", ".github/rulesets/tags.json"];

pub(crate) fn run(root: &Path, args: &[String]) -> Result<(), String> {
    if !args.is_empty() {
        return Err("usage: cargo xtask protect".into());
    }
    let repo = gh(
        root,
        &[
            "repo",
            "view",
            "--json",
            "nameWithOwner",
            "--jq",
            ".nameWithOwner",
        ],
    )?;
    let rulesets = format!("repos/{repo}/rulesets");
    let listed: Value = serde_json::from_str(&gh(root, &["api", &rulesets])?)
        .map_err(|e| format!("GitHub's list of rulesets: {e}"))?;

    for path in RULESETS {
        let file = root.join(path);
        let ruleset: Value = serde_json::from_str(
            &fs::read_to_string(&file).map_err(|e| format!("{}: {e}", file.display()))?,
        )
        .map_err(|e| format!("{path}: {e}"))?;
        let name = ruleset["name"]
            .as_str()
            .ok_or_else(|| format!("{path} has no name"))?;
        let existing = listed
            .as_array()
            .into_iter()
            .flatten()
            .find(|r| r["name"] == name)
            .and_then(|r| r["id"].as_u64());
        let (method, endpoint, done) = match existing {
            Some(id) => ("PUT", format!("{rulesets}/{id}"), "brought in line with"),
            None => ("POST", rulesets.clone(), "created from"),
        };
        let input = file.to_str().ok_or("a ruleset's path isn't valid UTF-8")?;
        gh(
            root,
            &[
                "api", "--method", method, &endpoint, "--input", input, "--silent",
            ],
        )?;
        println!("✓ {repo}: the {name:?} ruleset is {done} {path}.");
    }
    Ok(())
}

/// Runs the GitHub CLI in the checkout, and returns what it printed.
fn gh(root: &Path, args: &[&str]) -> Result<String, String> {
    let out = Command::new("gh")
        .args(args)
        .current_dir(root)
        .output()
        .map_err(|e| {
            format!("couldn't run gh ({e}): install the GitHub CLI from https://cli.github.com")
        })?;
    if !out.status.success() {
        return Err(explain(String::from_utf8_lossy(&out.stderr).trim()));
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_owned())
}

/// GitHub's refusal, and what to do about it.
fn explain(refusal: &str) -> String {
    if refusal.contains("Upgrade to GitHub Pro") {
        format!(
            "GitHub only applies rulesets to a public repository, or to a private one on \
             GitHub Pro. It said: {refusal}\n  Run this again once the repository is public."
        )
    } else {
        format!("gh: {refusal}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ruleset() -> Value {
        serde_json::from_str(include_str!("../../.github/rulesets/main.json")).unwrap()
    }

    fn tags() -> Value {
        serde_json::from_str(include_str!("../../.github/rulesets/tags.json")).unwrap()
    }

    /// The one exemption, in both: the repository's admin role, 5 in
    /// GitHub's API.
    fn only_the_admin_bypasses(ruleset: &Value) {
        let bypass = ruleset["bypass_actors"].as_array().unwrap();
        assert_eq!(bypass.len(), 1);
        assert_eq!(bypass[0]["actor_type"], "RepositoryRole");
        assert_eq!(bypass[0]["actor_id"], 5);
    }

    fn rule<'a>(ruleset: &'a Value, kind: &str) -> Option<&'a Value> {
        ruleset["rules"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["type"] == kind)
    }

    #[test]
    fn main_is_protected_and_only_the_admin_changes_it() {
        let ruleset = ruleset();
        assert_eq!(ruleset["target"], "branch");
        assert_eq!(ruleset["enforcement"], "active");
        assert_eq!(
            ruleset["conditions"]["ref_name"]["include"][0],
            "~DEFAULT_BRANCH"
        );
        for kind in ["deletion", "non_fast_forward", "update", "pull_request"] {
            assert!(rule(&ruleset, kind).is_some(), "no {kind} rule");
        }
        only_the_admin_bypasses(&ruleset);
    }

    #[test]
    fn only_the_admin_makes_moves_or_deletes_a_tag() {
        let tags = tags();
        assert_eq!(tags["target"], "tag");
        assert_eq!(tags["enforcement"], "active");
        assert_eq!(tags["conditions"]["ref_name"]["include"][0], "~ALL");
        for kind in ["creation", "update", "deletion"] {
            assert!(rule(&tags, kind).is_some(), "no {kind} rule");
        }
        only_the_admin_bypasses(&tags);
        assert_ne!(
            tags["name"],
            ruleset()["name"],
            "names tell them apart on GitHub"
        );
    }

    /// A workflow's jobs, by id, each with its lines.
    fn jobs(workflow: &str) -> Vec<(&str, Vec<&str>)> {
        let (_, body) = workflow.split_once("\njobs:\n").unwrap();
        let mut jobs: Vec<(&str, Vec<&str>)> = Vec::new();
        for line in body.lines() {
            // Each job starts with its id, two spaces in: `  lint:`.
            let id = line
                .strip_prefix("  ")
                .and_then(|l| l.strip_suffix(':'))
                .filter(|id| id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-'));
            match (id, jobs.last_mut()) {
                (Some(id), _) => jobs.push((id, Vec::new())),
                (None, Some((_, lines))) => lines.push(line),
                (None, None) => {}
            }
        }
        jobs
    }

    /// The one check the rules require is "Tests passed", the last job in
    /// tests.yml, which passes once every other job there has. A job it
    /// didn't wait for could fail without stopping a merge, so it waits for
    /// all of them.
    #[test]
    fn the_required_check_waits_for_every_job_in_tests() {
        let ruleset = ruleset();
        let required: Vec<&str> = rule(&ruleset, "required_status_checks").unwrap()["parameters"]
            ["required_status_checks"]
            .as_array()
            .unwrap()
            .iter()
            .map(|check| check["context"].as_str().unwrap())
            .collect();
        assert_eq!(required, ["Tests passed"]);

        let jobs = jobs(include_str!("../../.github/workflows/tests.yml"));
        let (last, lines) = jobs.last().unwrap();
        assert!(
            lines.contains(&"    name: Tests passed"),
            "the last job, {last}, isn't Tests passed"
        );
        let mut waits_for: Vec<&str> = lines
            .iter()
            .skip_while(|line| **line != "    needs:")
            .skip(1)
            .map_while(|line| line.strip_prefix("      - "))
            .collect();
        let mut others: Vec<&str> = jobs
            .iter()
            .map(|(id, _)| *id)
            .filter(|id| id != last)
            .collect();
        waits_for.sort_unstable();
        others.sort_unstable();
        assert_eq!(waits_for, others);
    }

    #[test]
    fn a_refusal_for_a_private_repository_says_what_to_do() {
        let said = explain(
            "Upgrade to GitHub Pro or make this repository public to enable this feature. (HTTP 403)",
        );
        assert!(said.contains("once the repository is public"), "{said}");
        assert_eq!(explain("not found"), "gh: not found");
    }
}
