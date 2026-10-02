use crate::{UpdatedBy, Version};

/// What a look for a newer release found, and did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpdateEvent {
    /// This release was downloaded, checked and put in place of the running
    /// copy: it runs from the next start.
    Installed(Version),
    /// This release is out, for `by` to get: Homebrew or cargo, which look
    /// after this copy, or the install line again, as steamcards couldn't
    /// put it in place itself.
    Available { version: Version, by: UpdatedBy },
}
