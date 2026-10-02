//! The part of a GitHub release's notes that the release workflow writes: how
//! to install or update, and how to check a download. GitHub adds the rest,
//! below it: the pull requests merged since the last release (grouped as
//! `.github/release.yml` says), and a link to everything that changed.

/// How to install or update a release, and how to check a download.
/// `attested` adds GitHub's attestation of where each archive was built.
pub(super) fn release_notes(repository: &str, attested: bool) -> String {
    let name = repository.trim_start_matches("https://github.com/");
    let raw = format!("https://raw.githubusercontent.com/{name}/main");
    let mut out = format!(
        "## Install or update\n\n\
         macOS and Linux:\n\n```sh\ncurl -fsSL {raw}/install/install.sh | sh\n```\n\n\
         Windows, in PowerShell:\n\n```powershell\nirm {raw}/install/install.ps1 | iex\n```\n\n\
         Homebrew: `brew upgrade steamcards`, or see the [README]({repository}#install) to \
         install it.\n\nAlready installed? The same command updates it.\n"
    );
    out.push_str(
        "\n## Check a download\n\n`SHA256SUMS` has every archive's checksum, which the \
         install commands check for you.",
    );
    if attested {
        out.push_str(&format!(
            " GitHub also attests that each archive was built by this repository's release \
             workflow:\n\n```sh\ngh attestation verify steamcards-x86_64-unknown-linux-musl.tar.gz --repo {name}\n```\n"
        ));
    } else {
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const REPO: &str = "https://github.com/o/r";

    #[test]
    fn a_releases_notes_say_how_to_install_and_check_it() {
        let notes = release_notes(REPO, true);

        assert!(notes.starts_with("## Install or update\n"));
        assert!(notes.contains(
            "curl -fsSL https://raw.githubusercontent.com/o/r/main/install/install.sh | sh"
        ));
        assert!(
            notes.contains(
                "irm https://raw.githubusercontent.com/o/r/main/install/install.ps1 | iex"
            )
        );
        assert!(notes.contains("brew upgrade steamcards"));
        assert!(notes.contains("gh attestation verify"));

        let unattested = release_notes(REPO, false);
        assert!(!unattested.contains("gh attestation"));
    }
}
