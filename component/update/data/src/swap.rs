use std::{fs, io, path::Path};

/// Puts `program` in place of the one at `exe`, which may be running: that
/// carries on as it was, and the next start runs the new one. It's written
/// beside it first, so a failure leaves the old one as it was.
pub(crate) fn put_in_place(exe: &Path, program: &[u8]) -> io::Result<()> {
    let new = exe.with_extension("new");
    fs::write(&new, program)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&new, fs::Permissions::from_mode(0o755))?;
    }
    replace(&new, exe).inspect_err(|_| {
        let _ = fs::remove_file(&new);
    })
}

/// The new one takes the old one's name: a running program keeps the file
/// it started from.
#[cfg(not(windows))]
fn replace(new: &Path, exe: &Path) -> io::Result<()> {
    fs::rename(new, exe)
}

/// Windows won't let a running program be replaced, but lets it be renamed:
/// it steps aside, and goes the next time a release is put in place.
#[cfg(windows)]
fn replace(new: &Path, exe: &Path) -> io::Result<()> {
    let old = exe.with_extension("old");
    let _ = fs::remove_file(&old);
    fs::rename(exe, &old)?;
    fs::rename(new, exe).inspect_err(|_| {
        let _ = fs::rename(&old, exe);
    })
}
