//! Semantic versions: reading them, ordering them, and working out the next.

use std::{cmp::Ordering, fmt};

/// A semantic version: `1.2.3`, or a pre-release like `1.2.3-rc.1`.
#[derive(Debug, PartialEq, Eq)]
pub(super) struct Version {
    core: [u64; 3],
    pre: Vec<String>,
}

/// How big a change a release is.
#[derive(Debug, Clone, Copy)]
pub(super) enum Bump {
    /// Fixes only: 1.2.3 → 1.2.4.
    Patch,
    /// New features: 1.2.3 → 1.3.0.
    Minor,
    /// Breaking changes: 1.2.3 → 2.0.0.
    Major,
}

impl Bump {
    pub(super) fn from_word(word: &str) -> Option<Self> {
        match word {
            "patch" => Some(Self::Patch),
            "minor" => Some(Self::Minor),
            "major" => Some(Self::Major),
            _ => None,
        }
    }
}

impl Version {
    pub(super) fn parse(text: &str) -> Result<Self, String> {
        let bad = || format!("{text:?} isn't a version like 1.2.3 or 1.2.3-rc.1");
        let (core, pre) = match text.split_once('-') {
            Some((core, pre)) => (core, Some(pre)),
            None => (text, None),
        };
        let numbers: Vec<u64> = core
            .split('.')
            .map(|n| {
                let leading_zero = n.len() > 1 && n.starts_with('0');
                if leading_zero { None } else { n.parse().ok() }
            })
            .collect::<Option<_>>()
            .ok_or_else(bad)?;
        let pre: Vec<String> =
            pre.map_or_else(Vec::new, |p| p.split('.').map(str::to_owned).collect());
        let fine = |id: &String| {
            !id.is_empty() && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
        };
        if !pre.iter().all(fine) {
            return Err(bad());
        }
        Ok(Self {
            core: numbers.try_into().map_err(|_| bad())?,
            pre,
        })
    }

    pub(super) fn is_pre_release(&self) -> bool {
        !self.pre.is_empty()
    }

    /// Which comes first, by semver's rules: numbers compare as numbers, and a
    /// pre-release comes before its release.
    pub(super) fn precedence(&self, other: &Self) -> Ordering {
        self.core
            .cmp(&other.core)
            .then_with(|| match (self.pre.is_empty(), other.pre.is_empty()) {
                (true, true) => Ordering::Equal,
                (true, false) => Ordering::Greater,
                (false, true) => Ordering::Less,
                (false, false) => self
                    .pre
                    .iter()
                    .zip(&other.pre)
                    .map(|(a, b)| match (a.parse::<u64>(), b.parse::<u64>()) {
                        (Ok(a), Ok(b)) => a.cmp(&b),
                        (Ok(_), Err(_)) => Ordering::Less,
                        (Err(_), Ok(_)) => Ordering::Greater,
                        (Err(_), Err(_)) => a.cmp(b),
                    })
                    .find(|o| o.is_ne())
                    .unwrap_or_else(|| self.pre.len().cmp(&other.pre.len())),
            })
    }

    /// The version after this one. A pre-release becomes its own release when
    /// the bump is the size it was heading for: 2.0.0-rc.1 bumped by major is
    /// 2.0.0, and 1.3.0-rc.1 bumped by minor is 1.3.0.
    pub(super) fn bumped(&self, bump: Bump) -> Self {
        let [major, minor, patch] = self.core;
        let pre = self.is_pre_release();
        let core = match bump {
            Bump::Patch if pre => [major, minor, patch],
            Bump::Patch => [major, minor, patch + 1],
            Bump::Minor if pre && patch == 0 => [major, minor, 0],
            Bump::Minor => [major, minor + 1, 0],
            Bump::Major if pre && minor == 0 && patch == 0 => [major, 0, 0],
            Bump::Major => [major + 1, 0, 0],
        };
        Self {
            core,
            pre: Vec::new(),
        }
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let [major, minor, patch] = self.core;
        write!(f, "{major}.{minor}.{patch}")?;
        if self.is_pre_release() {
            write!(f, "-{}", self.pre.join("."))?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(text: &str) -> Version {
        Version::parse(text).unwrap()
    }

    #[test]
    fn versions_are_read_and_written_back_the_same() {
        for text in [
            "0.1.0",
            "1.2.3",
            "10.20.30",
            "1.0.0-rc.1",
            "1.0.0-alpha.beta-2",
        ] {
            assert_eq!(v(text).to_string(), text);
        }
        assert!(v("1.0.0-rc.1").is_pre_release());
        assert!(!v("1.0.0").is_pre_release());
    }

    #[test]
    fn anything_else_is_refused() {
        for bad in [
            "",
            "1.2",
            "1.2.3.4",
            "01.2.3",
            "1.2.x",
            "v1.2.3",
            "1.2.3-",
            "1.2.3-rc..1",
            "1.2.3-rc_1",
        ] {
            assert!(Version::parse(bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn versions_are_ordered_by_semvers_rules() {
        let ordered = [
            "0.9.0",
            "0.10.0",
            "1.0.0-alpha",
            "1.0.0-alpha.1",
            "1.0.0-alpha.beta",
            "1.0.0-beta.2",
            "1.0.0-beta.11",
            "1.0.0-rc.1",
            "1.0.0",
            "1.0.1",
        ];
        for pair in ordered.windows(2) {
            assert_eq!(
                v(pair[0]).precedence(&v(pair[1])),
                Ordering::Less,
                "{pair:?}"
            );
            assert_eq!(
                v(pair[1]).precedence(&v(pair[0])),
                Ordering::Greater,
                "{pair:?}"
            );
        }
        assert_eq!(v("1.0.0").precedence(&v("1.0.0")), Ordering::Equal);
    }

    #[test]
    fn a_bump_works_out_the_next_version() {
        let cases = [
            ("0.1.0", Bump::Patch, "0.1.1"),
            ("0.1.0", Bump::Minor, "0.2.0"),
            ("0.1.0", Bump::Major, "1.0.0"),
            ("1.2.3", Bump::Minor, "1.3.0"),
            ("1.2.3", Bump::Major, "2.0.0"),
            // A pre-release is released by the bump it was heading for.
            ("1.2.4-rc.1", Bump::Patch, "1.2.4"),
            ("1.3.0-rc.1", Bump::Minor, "1.3.0"),
            ("2.0.0-rc.1", Bump::Major, "2.0.0"),
            // A bigger bump than that moves past it.
            ("1.2.4-rc.1", Bump::Minor, "1.3.0"),
            ("1.3.0-rc.1", Bump::Major, "2.0.0"),
        ];
        for (from, bump, to) in cases {
            assert_eq!(v(from).bumped(bump).to_string(), to, "{from} {bump:?}");
        }
        assert!(Bump::from_word("minor").is_some() && Bump::from_word("tiny").is_none());
    }
}
