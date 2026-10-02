//! Versions: reading them, ordering them, and working out the next.

use std::fmt;

/// A version, three numbers: `1.2.3`. Versions are ordered by their numbers,
/// the first first, so `0.10.0` comes after `0.9.0`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct Version([u64; 3]);

impl Version {
    pub(super) fn parse(text: &str) -> Result<Self, String> {
        let bad = || format!("{text:?} isn't a version like 1.2.3");
        let numbers: Vec<u64> = text
            .split('.')
            .map(|n| {
                let digits = !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit());
                let leading_zero = n.len() > 1 && n.starts_with('0');
                if digits && !leading_zero {
                    n.parse().ok()
                } else {
                    None
                }
            })
            .collect::<Option<_>>()
            .ok_or_else(bad)?;
        Ok(Self(numbers.try_into().map_err(|_| bad())?))
    }

    /// The version after this one: its last number one higher, so 0.1.0 is
    /// followed by 0.1.1.
    pub(super) fn next(self) -> Self {
        let [first, second, last] = self.0;
        Self([first, second, last + 1])
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let [first, second, last] = self.0;
        write!(f, "{first}.{second}.{last}")
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
        for text in ["0.1.0", "1.2.3", "10.20.30"] {
            assert_eq!(v(text).to_string(), text);
        }
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
            "+1.2.3",
            "1.2.3-rc.1",
            "1..3",
        ] {
            assert!(Version::parse(bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn versions_are_ordered_by_their_numbers() {
        let ordered = ["0.1.0", "0.1.1", "0.9.0", "0.10.0", "1.0.0", "1.0.1"];
        for pair in ordered.windows(2) {
            assert!(v(pair[0]) < v(pair[1]), "{pair:?}");
        }
        assert_eq!(v("1.0.0"), v("1.0.0"));
    }

    #[test]
    fn the_next_version_counts_the_last_number_up() {
        assert_eq!(v("0.1.0").next(), v("0.1.1"));
        assert_eq!(v("0.1.9").next(), v("0.1.10"));
        assert_eq!(v("1.4.2").next(), v("1.4.3"));
    }
}
