use update::{UpdateEvent, UpdatedBy};

/// What keeping steamcards up to date found, as the log says it: a release
/// put in place, or one that's out, and how to get it.
pub fn update(event: &UpdateEvent) -> String {
    match *event {
        UpdateEvent::Installed(version) => {
            format!("steamcards {version} is installed: it runs from the next start.")
        }
        UpdateEvent::Available { version, by } => {
            let how = match by {
                UpdatedBy::Homebrew => "brew upgrade steamcards",
                UpdatedBy::Cargo => "build it again from source to update",
                UpdatedBy::Itself => "run the install line again to get it",
            };
            format!("steamcards {version} is out: {how}.")
        }
    }
}

#[cfg(test)]
mod tests {
    use update::Version;

    use super::*;

    const NEXT: Version = Version::new(0, 1, 3);

    #[test]
    fn each_update_says_what_to_do_about_it() {
        assert_eq!(
            update(&UpdateEvent::Installed(NEXT)),
            "steamcards 0.1.3 is installed: it runs from the next start."
        );
        assert_eq!(
            update(&UpdateEvent::Available {
                version: NEXT,
                by: UpdatedBy::Homebrew
            }),
            "steamcards 0.1.3 is out: brew upgrade steamcards."
        );
        assert_eq!(
            update(&UpdateEvent::Available {
                version: NEXT,
                by: UpdatedBy::Itself
            }),
            "steamcards 0.1.3 is out: run the install line again to get it."
        );
    }
}
