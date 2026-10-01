//! The part of a GitHub release's notes that the release workflow writes: how
//! to install or update, and how to check a download. GitHub adds the rest,
//! below it: the pull requests merged since the last release (grouped as
//! `.github/release.yml` says), and a link to everything that changed.

use super::version::Version;

/// How to install or update `version`, and how to check a download.
/// `attested` adds GitHub's attestation of where each archive was built.
pub(super) fn release_notes(version: &Version, repository: &str, attested: bool) -> String {
    let name = repository.trim_start_matches("https://github.com/");
    let raw = format!("https://raw.githubusercontent.com/{name}/main");
    // A pre-release is installed only when asked for by version.
    let (sh, ps) = if version.is_pre_release() {
        (
            format!("curl -fsSL {raw}/install/install.sh | STEAMCARDS_VERSION={version} sh"),
            format!("$env:STEAMCARDS_VERSION = '{version}'; irm {raw}/install/install.ps1 | iex"),
        )
    } else {
        (
            format!("curl -fsSL {raw}/install/install.sh | sh"),
            format!("irm {raw}/install/install.ps1 | iex"),
        )
    };

    let mut out = format!(
        "## Install or update\n\n\
         macOS and Linux:\n\n```sh\n{sh}\n```\n\n\
         Windows, in PowerShell:\n\n```powershell\n{ps}\n```\n\n"
    );
    if version.is_pre_release() {
        out.push_str(
            "This is a pre-release: the commands above install it by name, and neither they \
             nor Homebrew offer it otherwise.\n",
        );
    } else {
        out.push_str(&format!(
            "Homebrew: `brew upgrade steamcards`, or see the [README]({repository}#install) to \
             install it.\n\nAlready installed? The same command updates it.\n"
        ));
    }
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

    fn v(text: &str) -> Version {
        Version::parse(text).unwrap()
    }

    #[test]
    fn a_releases_notes_say_how_to_install_and_check_it() {
        let notes = release_notes(&v("0.1.0"), REPO, true);

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

        let unattested = release_notes(&v("0.1.0"), REPO, false);
        assert!(!unattested.contains("gh attestation"));
    }

    #[test]
    fn a_pre_releases_notes_install_it_by_name() {
        let notes = release_notes(&v("0.2.0-rc.1"), REPO, false);

        assert!(notes.contains("| STEAMCARDS_VERSION=0.2.0-rc.1 sh"));
        assert!(notes.contains("$env:STEAMCARDS_VERSION = '0.2.0-rc.1'; irm"));
        assert!(!notes.contains("brew upgrade"));
    }
}
