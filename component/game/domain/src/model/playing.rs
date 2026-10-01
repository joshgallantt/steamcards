use crate::AppId;

/// Who plays on the account, as far as this session goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Playing {
    /// This session: what it was asked to play, which may be nothing.
    Here,
    /// Another device, playing this game when Steam says. While it does,
    /// this session plays nothing: Steam would sign it off.
    Elsewhere(Option<AppId>),
}
