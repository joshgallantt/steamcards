use std::fmt;

/// A version of steamcards, three numbers: `0.1.2`. Versions are ordered by
/// their numbers, the first first, so `0.10.0` comes after `0.9.0`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Version([u64; 3]);

impl Version {
    pub const fn new(first: u64, second: u64, last: u64) -> Self {
        Self([first, second, last])
    }

    /// Reads `1.2.3`, or `None` for anything else: no `v` in front, and no
    /// pre-release after.
    pub fn parse(text: &str) -> Option<Self> {
        let numbers: Vec<u64> = text
            .split('.')
            .map(|n| {
                let digits = !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit());
                let leading_zero = n.len() > 1 && n.starts_with('0');
                (digits && !leading_zero).then(|| n.parse().ok()).flatten()
            })
            .collect::<Option<_>>()?;
        Some(Self(numbers.try_into().ok()?))
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

    #[test]
    fn a_version_is_three_numbers_ordered_as_numbers() {
        let v = |text| Version::parse(text).unwrap();
        assert_eq!(v("0.1.2"), Version::new(0, 1, 2));
        assert_eq!(v("10.20.30").to_string(), "10.20.30");
        assert!(v("0.9.0") < v("0.10.0"));
        assert!(v("0.1.9") < v("1.0.0"));
        for bad in [
            "",
            "1.2",
            "1.2.3.4",
            "01.2.3",
            "v1.2.3",
            "1.2.3-rc.1",
            "+1.2.3",
        ] {
            assert_eq!(Version::parse(bad), None, "{bad:?}");
        }
    }
}
