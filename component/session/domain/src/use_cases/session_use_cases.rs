//! What can be asked of the session. Each use case is a value: its type says
//! what it takes and gives, and a constructor builds the real one.

use std::sync::Arc;

use crate::SessionKeeper;

/// Ends the farming session: the farmer's next run starts a new one, with no
/// drops, no hours counted and nothing set aside. Signing out ends it, and
/// so does signing in as another account.
pub type EndSession = Arc<dyn Fn() + Send + Sync>;

pub fn end_session(sessions: Arc<SessionKeeper>) -> EndSession {
    Arc::new(move || sessions.end())
}
