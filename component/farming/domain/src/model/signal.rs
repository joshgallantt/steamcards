use session::NewItem;
use steam_library::AppId;

/// Something Steam said that the farmer acts on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Signal {
    /// Another device started playing on the account (what, when Steam
    /// says): nothing played here counts until it stops.
    Blocked(Option<AppId>),
    /// It stopped.
    Unblocked,
    /// Another device took over playing, and Steam signed this session off
    /// to let it: nothing played here counts until it stops.
    TakenOver,
    /// New items arrived: a card may have dropped. The items Steam listed,
    /// each once; none when it gave only a count.
    NewItems(Vec<NewItem>),
    /// Another session signed on in this one's place. Signing on again would
    /// knock that one off in turn.
    Replaced,
    /// The connection to Steam went, for this reason.
    Lost(String),
}
