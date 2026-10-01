use crate::AppId;

/// Something Steam said about playing on the account.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlayingSignal {
    /// Another device started playing on the account (what, when Steam
    /// says): nothing played here counts until it stops.
    Blocked(Option<AppId>),
    /// It stopped.
    Unblocked,
    /// Another device took over playing, and Steam signed this session off
    /// to let it: nothing played here counts until it stops.
    TakenOver,
    /// Another session signed on in this one's place. Signing on again would
    /// knock that one off in turn.
    Replaced,
    /// The connection to Steam went, for this reason.
    Lost(String),
}
