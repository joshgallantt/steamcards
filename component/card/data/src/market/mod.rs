//! The Steam market on steamcommunity.com, in its own terms: a game's cards
//! with their lowest listings (`search/render`), read as Steam's own pages,
//! SteamDB's extension and Steam Economy Enhancer read them (research §1.1). Only prices read the market,
//! so it's read here, beside them.
//!
//! The market limits requests harder than the rest of the site, and meets
//! quick retries by blocking for longer. So every market request goes
//! through one queue, one at a time, spaced out (research §1.3): signed in,
//! 5 seconds apart and up to a second more; signed out, 12 seconds. A 429
//! pauses every market request for 10 minutes, and nothing is asked
//! meanwhile; then one request goes to see, and if that's turned down too,
//! the pause doubles, to an hour at most. A server error is asked once more,
//! 30 seconds later. The queue sits on top of the site's own gap between
//! requests, never in place of it, and the site's quick retries never apply.
//!
//! A market that couldn't be asked, or didn't answer (no sign-in, no
//! network, a server error twice), is told apart from an answer that can't
//! be used: nothing is wrong with what was asked for, and it's worth asking
//! again soon.

mod market_pace;
mod market_queue;
mod search;

pub use market_pace::MarketPace;
pub(crate) use market_queue::MarketQueue;
pub(crate) use search::{Listed, MAX_SET_PAGES, read_search, search_path};
