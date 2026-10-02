use std::path::Path;

use update::UpdatedBy;

/// Who updates the copy of steamcards at `exe`, as where it is says:
/// Homebrew keeps what it installs in its `Cellar`, and cargo builds into
/// `<cargo home>/bin`. Any other copy updates itself, as the install
/// scripts' does.
pub fn updated_by(exe: &Path, cargo_home: &Path) -> UpdatedBy {
    if exe.components().any(|c| c.as_os_str() == "Cellar") {
        UpdatedBy::Homebrew
    } else if exe.parent() == Some(cargo_home.join("bin").as_path()) {
        UpdatedBy::Cargo
    } else {
        UpdatedBy::Itself
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn where_a_copy_is_says_who_updates_it() {
        let cargo = Path::new("/home/me/.cargo");
        let by = |exe: &str| updated_by(Path::new(exe), cargo);

        assert_eq!(
            by("/opt/homebrew/Cellar/steamcards/0.1.2/bin/steamcards"),
            UpdatedBy::Homebrew
        );
        assert_eq!(
            by("/home/linuxbrew/.linuxbrew/Cellar/steamcards/0.1.2/bin/steamcards"),
            UpdatedBy::Homebrew
        );
        assert_eq!(by("/home/me/.cargo/bin/steamcards"), UpdatedBy::Cargo);
        assert_eq!(by("/home/me/.local/bin/steamcards"), UpdatedBy::Itself);
    }
}
