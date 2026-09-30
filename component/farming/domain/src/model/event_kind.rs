/// What a log line is about, so the UI can highlight the ones that matter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EventKind {
    #[default]
    Info,
    /// Routine looks at the cards.
    Progress,
    /// Started playing something.
    Playing,
    /// Moved on to something else before it was done.
    Switched,
    /// A card dropped.
    Dropped,
    /// A card that dropped, named a moment later.
    Identified,
    Warning,
    Error,
}
