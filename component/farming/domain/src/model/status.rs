/// What the farmer is doing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Status {
    #[default]
    Idle,
    /// Reading the library, or connecting.
    Checking,
    Farming,
    /// Another device is playing on the account; farming waits for it.
    Blocked,
    Error,
}
