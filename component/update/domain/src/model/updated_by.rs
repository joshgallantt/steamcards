/// Who updates this copy of steamcards, as where it's installed says.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpdatedBy {
    /// steamcards itself: the install scripts put this copy in place, or the
    /// user did, by hand. It puts a new release in place of itself.
    Itself,
    /// Homebrew, which installed it: `brew upgrade steamcards`.
    Homebrew,
    /// cargo: it was built from source, and is built again to update.
    Cargo,
}
