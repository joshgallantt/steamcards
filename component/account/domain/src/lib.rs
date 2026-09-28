//! Account: the one Steam account steamcards farms for.
//!
//! Tokens, login IDs and connections are not here — how a sign-in is kept is
//! a data concern. The domain knows only whether the account is signed in,
//! what it's called, and whether Steam has stopped taking the sign-in.

mod model;
mod repository;
mod use_cases;

#[cfg(feature = "test-support")]
pub mod test_support;

pub use model::{Account, LinkError, LoginChallenge, UnlinkError};
pub use repository::AccountRepository;
pub use use_cases::{
    GetAccount, LinkAccount, RefreshAccount, UnlinkAccount, get_account, link_account,
    refresh_account, unlink_account,
};
