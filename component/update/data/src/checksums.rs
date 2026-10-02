use anyhow::anyhow;
use sha2::{Digest, Sha256};

/// Whether `file`, named `name`, is what the release's `SHA256SUMS` lists:
/// a line of its SHA-256, two spaces, and its name.
pub(crate) fn check(file: &[u8], name: &str, sums: &str) -> anyhow::Result<()> {
    let listed = sums
        .lines()
        .find_map(|line| {
            let (sum, listed_name) = line.split_once("  ")?;
            (listed_name.trim() == name).then(|| sum.trim().to_ascii_lowercase())
        })
        .ok_or_else(|| anyhow!("the release's SHA256SUMS doesn't list {name}"))?;
    if hex::encode(Sha256::digest(file)) == listed {
        Ok(())
    } else {
        Err(anyhow!(
            "{name} isn't what the release's SHA256SUMS says it is"
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_file_is_checked_against_its_line() {
        let sums = "\
2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824  hello.tar.gz
0000000000000000000000000000000000000000000000000000000000000000  other.zip
";
        assert!(check(b"hello", "hello.tar.gz", sums).is_ok());
        assert!(check(b"hullo", "hello.tar.gz", sums).is_err(), "changed");
        assert!(check(b"hello", "missing.tar.gz", sums).is_err(), "unlisted");
    }
}
