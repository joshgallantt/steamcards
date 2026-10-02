use std::{ffi::OsStr, io::Read};

use anyhow::{Context, anyhow};
use flate2::read::GzDecoder;

/// What a release holds for one computer: its archive, and the program in
/// it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Asset {
    /// The archive's name in the release, like
    /// `steamcards-aarch64-apple-darwin.tar.gz`.
    pub archive: String,
    /// The program's name in the archive: `steamcards`, or `steamcards.exe`.
    pub program: String,
}

impl Asset {
    /// This computer's, as the release workflow names them; `None` where
    /// there's no build.
    pub fn for_this_computer() -> Option<Self> {
        let target = match (std::env::consts::OS, std::env::consts::ARCH) {
            ("macos", "aarch64") => "aarch64-apple-darwin",
            ("macos", "x86_64") => "x86_64-apple-darwin",
            ("linux", "x86_64") => "x86_64-unknown-linux-musl",
            ("linux", "aarch64") => "aarch64-unknown-linux-musl",
            // Windows on ARM runs the x64 build, as the install script has it.
            ("windows", "x86_64" | "aarch64") => "x86_64-pc-windows-msvc",
            _ => return None,
        };
        let windows = std::env::consts::OS == "windows";
        Some(Self {
            archive: format!(
                "steamcards-{target}.{}",
                if windows { "zip" } else { "tar.gz" }
            ),
            program: if windows {
                "steamcards.exe"
            } else {
                "steamcards"
            }
            .to_owned(),
        })
    }

    /// The program, out of the archive.
    pub(crate) fn program_in(&self, archive: &[u8]) -> anyhow::Result<Vec<u8>> {
        if self.archive.ends_with(".zip") {
            zip_program(archive, &self.program)
        } else {
            tar_gz_program(archive, &self.program)
        }
    }
}

fn tar_gz_program(archive: &[u8], name: &str) -> anyhow::Result<Vec<u8>> {
    let unreadable = "couldn't read the release's archive";
    let mut tar = tar::Archive::new(GzDecoder::new(archive));
    for entry in tar.entries().context(unreadable)? {
        let mut entry = entry.context(unreadable)?;
        let named = entry
            .path()
            .ok()
            .is_some_and(|p| p.file_name() == Some(OsStr::new(name)));
        if named {
            let mut program = Vec::new();
            entry.read_to_end(&mut program).context(unreadable)?;
            return Ok(program);
        }
    }
    Err(anyhow!("the release's archive has no {name} in it"))
}

#[cfg(windows)]
fn zip_program(archive: &[u8], name: &str) -> anyhow::Result<Vec<u8>> {
    let unreadable = "couldn't read the release's archive";
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(archive)).context(unreadable)?;
    let mut file = zip
        .by_name(name)
        .with_context(|| format!("the release's archive has no {name} in it"))?;
    let mut program = Vec::new();
    file.read_to_end(&mut program).context(unreadable)?;
    Ok(program)
}

#[cfg(not(windows))]
fn zip_program(_: &[u8], _: &str) -> anyhow::Result<Vec<u8>> {
    Err(anyhow!("only Windows' builds come as a zip"))
}
