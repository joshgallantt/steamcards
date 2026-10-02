//! Valve's binary KeyValues, the form Steam's product info gives a package's
//! details in: just enough of it to read which apps a package holds. The
//! format is as SteamKit reads it (`KeyValue.TryReadAsBinary`): each entry is
//! a byte saying what it is, its name, then its value; a section's entries
//! end with an end byte.

use anyhow::{anyhow, bail};

/// What an entry's first byte says it is.
const SECTION: u8 = 0;
const STRING: u8 = 1;
const INT32: u8 = 2;
const FLOAT32: u8 = 3;
const POINTER: u8 = 4;
const COLOR: u8 = 6;
const UINT64: u8 = 7;
const END: u8 = 8;
const INT64: u8 = 10;
const ALTERNATE_END: u8 = 11;

/// Sections nest no deeper than this in a package's details.
const MAX_DEPTH: usize = 16;

#[derive(Debug, Clone, PartialEq)]
enum Value {
    Section(Vec<(String, Value)>),
    Text(String),
    Number(i64),
    /// A float, or a number too big to be an app: nothing steamcards reads.
    Other,
}

/// The apps a package holds, from its product info: a number the Steam
/// client checks, then its KeyValues, a section named for the package with
/// its apps listed under `appids`.
pub(crate) fn package_apps(buffer: &[u8]) -> anyhow::Result<Vec<u32>> {
    let kv = buffer
        .get(4..)
        .ok_or_else(|| anyhow!("a package's details too short to read"))?;
    let root = Reader { bytes: kv, at: 0 }.entries(0)?;
    let Some((_, Value::Section(package))) = root.first() else {
        bail!("a package's details with no package in them");
    };
    let apps = package.iter().find_map(|(key, value)| match value {
        Value::Section(apps) if key == "appids" => Some(apps),
        _ => None,
    });
    Ok(apps
        .into_iter()
        .flatten()
        .filter_map(|(_, value)| match value {
            Value::Number(id) => u32::try_from(*id).ok(),
            Value::Text(id) => id.parse().ok(),
            _ => None,
        })
        .collect())
}

struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl Reader<'_> {
    /// Entries up to the end of their section, or of the bytes.
    fn entries(&mut self, depth: usize) -> anyhow::Result<Vec<(String, Value)>> {
        if depth > MAX_DEPTH {
            bail!("KeyValues nested too deep to be a package's details");
        }
        let mut out = Vec::new();
        while let Some(kind) = self.byte() {
            if matches!(kind, END | ALTERNATE_END) {
                break;
            }
            let key = self.text()?;
            let value = match kind {
                SECTION => Value::Section(self.entries(depth + 1)?),
                STRING => Value::Text(self.text()?),
                INT32 | POINTER | COLOR => {
                    Value::Number(i64::from(i32::from_le_bytes(self.take()?)))
                }
                INT64 => Value::Number(i64::from_le_bytes(self.take()?)),
                UINT64 => i64::try_from(u64::from_le_bytes(self.take()?))
                    .map_or(Value::Other, Value::Number),
                FLOAT32 => {
                    self.take::<4>()?;
                    Value::Other
                }
                _ => bail!("KeyValues of a kind steamcards can't read ({kind})"),
            };
            out.push((key, value));
        }
        Ok(out)
    }

    fn byte(&mut self) -> Option<u8> {
        let b = *self.bytes.get(self.at)?;
        self.at += 1;
        Some(b)
    }

    fn take<const N: usize>(&mut self) -> anyhow::Result<[u8; N]> {
        let bytes: [u8; N] = self
            .bytes
            .get(self.at..self.at + N)
            .and_then(|b| b.try_into().ok())
            .ok_or_else(|| anyhow!("KeyValues cut short"))?;
        self.at += N;
        Ok(bytes)
    }

    /// Text up to its terminating zero.
    fn text(&mut self) -> anyhow::Result<String> {
        let rest = self.bytes.get(self.at..).unwrap_or_default();
        let end = rest
            .iter()
            .position(|&b| b == 0)
            .ok_or_else(|| anyhow!("KeyValues cut short"))?;
        self.at += end + 1;
        Ok(String::from_utf8_lossy(&rest[..end]).into_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One entry: what it is, its name, and its value's bytes.
    fn entry(kind: u8, key: &str, value: &[u8]) -> Vec<u8> {
        let mut out = vec![kind];
        out.extend(key.as_bytes());
        out.push(0);
        out.extend(value);
        out
    }

    fn section(key: &str, entries: &[Vec<u8>]) -> Vec<u8> {
        let mut inside = entries.concat();
        inside.push(END);
        entry(SECTION, key, &inside)
    }

    fn text(s: &str) -> Vec<u8> {
        let mut out = s.as_bytes().to_vec();
        out.push(0);
        out
    }

    /// A package's details as Steam's product info gives them.
    fn details(package: &[Vec<u8>]) -> Vec<u8> {
        let mut out = 1u32.to_le_bytes().to_vec();
        out.extend(section("12345", package));
        out.push(END);
        out
    }

    #[test]
    fn the_apps_a_package_holds_are_read_whatever_else_it_says() {
        let buffer = details(&[
            entry(INT32, "packageid", &12345i32.to_le_bytes()),
            entry(INT32, "billingtype", &10i32.to_le_bytes()),
            entry(STRING, "name", &text("Portal 2 + Soundtrack")),
            entry(FLOAT32, "weight", &1.5f32.to_le_bytes()),
            entry(UINT64, "huge", &u64::MAX.to_le_bytes()),
            section(
                "extended",
                &[entry(INT64, "expiry", &1_790_000_000i64.to_le_bytes())],
            ),
            section(
                "appids",
                &[
                    entry(INT32, "0", &620i32.to_le_bytes()),
                    entry(INT32, "1", &323_180i32.to_le_bytes()),
                ],
            ),
            section("depotids", &[entry(INT32, "0", &621i32.to_le_bytes())]),
        ]);

        assert_eq!(package_apps(&buffer).unwrap(), [620, 323_180]);
    }

    #[test]
    fn apps_written_as_text_are_read_too() {
        let buffer = details(&[section("appids", &[entry(STRING, "0", &text("620"))])]);
        assert_eq!(package_apps(&buffer).unwrap(), [620]);
    }

    #[test]
    fn a_package_with_no_apps_holds_none() {
        let buffer = details(&[entry(INT32, "packageid", &12345i32.to_le_bytes())]);
        assert_eq!(package_apps(&buffer).unwrap(), Vec::<u32>::new());
    }

    #[test]
    fn what_doesnt_read_says_so() {
        assert!(package_apps(&[1, 0]).is_err(), "too short");
        assert!(package_apps(&1u32.to_le_bytes()).is_err(), "no package");
        let whole = details(&[section(
            "appids",
            &[entry(INT32, "0", &620i32.to_le_bytes())],
        )]);
        assert!(
            package_apps(&whole[..whole.len() - 5]).is_err(),
            "cut short, in an app's number"
        );
        let unknown = details(&[entry(5, "wide", &[0, 0])]);
        assert!(package_apps(&unknown).is_err(), "a wide string");
        let mut deep = 1u32.to_le_bytes().to_vec();
        for _ in 0..=MAX_DEPTH + 1 {
            deep.extend([SECTION, b'x', 0]);
        }
        assert!(package_apps(&deep).is_err(), "nested too deep");
    }
}
