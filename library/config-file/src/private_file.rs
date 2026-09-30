use std::{fs, path::Path};

/// Writes `data` as the whole of the file at `path`, or nothing: to a file
/// beside it first, then renamed over it.
pub(crate) fn write_whole(path: &Path, data: &[u8]) -> anyhow::Result<()> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension("json.tmp");
    write_private(&tmp, data)?;
    fs::rename(&tmp, path)?;
    Ok(())
}

/// Writes `data` to `path`, readable and writable by its owner alone.
#[cfg(unix)]
fn write_private(path: &Path, data: &[u8]) -> std::io::Result<()> {
    use std::{io::Write, os::unix::fs::OpenOptionsExt};

    // A file left over from a failed write may have looser permissions.
    let _ = fs::remove_file(path);
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)?;
    file.write_all(data)
}

/// Writes `data` to `path`. Elsewhere than macOS and Linux, the folder's own
/// permissions decide who can read it.
#[cfg(not(unix))]
fn write_private(path: &Path, data: &[u8]) -> std::io::Result<()> {
    fs::write(path, data)
}
