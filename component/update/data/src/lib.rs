//! The update domain's contract, satisfied by GitHub and this computer: the
//! repository finds the latest release through a client, GitHub's, which
//! reads where a release page redirects to, as the install scripts do, and
//! downloads a release's files. A release is checked against its
//! `SHA256SUMS`, unpacked, and put in place of the running copy, which
//! carries on until it's next started. Imports `update` because the contract
//! is declared there; `update` imports nothing back.

mod archive;
mod checksums;
mod default_release_repository;
mod release_client;
mod swap;
mod updated_by;

pub use archive::Asset;
pub use default_release_repository::DefaultReleaseRepository;
pub use release_client::{GitHubReleaseClient, ReleaseClient};
pub use updated_by::updated_by;
