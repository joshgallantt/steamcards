//! steamcards' releases against a stand-in for GitHub: the latest found
//! where the releases page redirects, as the install scripts find it, and a
//! release checked against its `SHA256SUMS`, then put in place of a copy in
//! a folder of its own.

use std::{fs, path::PathBuf, sync::Arc};

use debug_log::DebugLog;
use flate2::{Compression, write::GzEncoder};
use sha2::{Digest, Sha256};
use update::{ReleaseRepository, Version};
use update_data::{Asset, DefaultReleaseRepository, GitHubReleaseClient};
use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{method, path},
};

const ARCHIVE: &str = "steamcards-test.tar.gz";
const NEXT: Version = Version::new(0, 1, 3);

fn asset() -> Asset {
    Asset {
        archive: ARCHIVE.into(),
        program: "steamcards".into(),
    }
}

/// A copy of steamcards, in a folder of its own.
fn copy(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("steamcards-update-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let exe = dir.join("steamcards");
    fs::write(&exe, b"steamcards 0.1.2").unwrap();
    exe
}

fn repository(github: &MockServer, exe: PathBuf) -> DefaultReleaseRepository {
    DefaultReleaseRepository::new(
        Arc::new(GitHubReleaseClient::new(&github.uri())),
        exe,
        Some(asset()),
        DebugLog::off(),
    )
}

/// A release's archive, as the release workflow packs one.
fn archive(program: &[u8]) -> Vec<u8> {
    let mut tar = tar::Builder::new(GzEncoder::new(Vec::new(), Compression::default()));
    for (name, body) in [("steamcards", program), ("LICENSE", b"MIT".as_slice())] {
        let mut header = tar::Header::new_gnu();
        header.set_size(body.len() as u64);
        header.set_mode(0o755);
        header.set_cksum();
        tar.append_data(&mut header, name, body).unwrap();
    }
    tar.into_inner().unwrap().finish().unwrap()
}

/// GitHub has release v0.1.3, with this archive and these checksums.
async fn released(github: &MockServer, archive: Vec<u8>, sums: String) {
    Mock::given(method("GET"))
        .and(path(format!("/download/v0.1.3/{ARCHIVE}")))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(archive))
        .mount(github)
        .await;
    Mock::given(method("GET"))
        .and(path("/download/v0.1.3/SHA256SUMS"))
        .respond_with(ResponseTemplate::new(200).set_body_string(sums))
        .mount(github)
        .await;
}

fn sums_of(archive: &[u8]) -> String {
    format!("{}  {ARCHIVE}\n", hex::encode(Sha256::digest(archive)))
}

#[tokio::test]
async fn the_latest_release_is_where_the_releases_page_sends_latest() {
    let github = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/latest"))
        .respond_with(
            ResponseTemplate::new(302)
                .insert_header("Location", format!("{}/tag/v0.1.3", github.uri())),
        )
        .mount(&github)
        .await;

    let latest = repository(&github, copy("latest")).latest().await.unwrap();

    assert_eq!(latest, Some(NEXT));
}

#[tokio::test]
async fn with_no_release_yet_there_is_none_to_be_had() {
    let github = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/latest"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&github)
        .await;

    let latest = repository(&github, copy("none")).latest().await.unwrap();

    assert_eq!(latest, None);
}

#[tokio::test]
async fn a_release_is_checked_and_put_in_place_for_the_next_start() {
    let github = MockServer::start().await;
    let archive = archive(b"steamcards 0.1.3");
    released(&github, archive.clone(), sums_of(&archive)).await;
    let exe = copy("install");

    repository(&github, exe.clone())
        .install(NEXT)
        .await
        .unwrap();

    assert_eq!(fs::read(&exe).unwrap(), b"steamcards 0.1.3");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = fs::metadata(&exe).unwrap().permissions().mode();
        assert_eq!(mode & 0o111, 0o111, "it runs");
    }
    assert!(!exe.with_extension("new").exists(), "nothing left over");
}

#[tokio::test]
async fn a_release_that_isnt_what_its_checksums_say_is_left_alone() {
    let github = MockServer::start().await;
    let archive = archive(b"steamcards 0.1.3");
    let wrong = sums_of(b"something else");
    released(&github, archive, wrong).await;
    let exe = copy("tampered");

    let put = repository(&github, exe.clone()).install(NEXT).await;

    assert!(put.is_err());
    assert_eq!(
        fs::read(&exe).unwrap(),
        b"steamcards 0.1.2",
        "the running copy as it was"
    );
}
