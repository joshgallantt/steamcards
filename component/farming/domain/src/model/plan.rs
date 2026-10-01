use game::AppId;

/// What to play next.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Plan {
    /// This game on its own, until its cards have dropped.
    Cards(AppId),
    /// These together, until the first of them has the hours its cards need.
    Hours(Vec<AppId>),
    /// Nothing is worth playing.
    Nothing,
}
