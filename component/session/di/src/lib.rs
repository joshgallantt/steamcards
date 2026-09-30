//! Where the session is wired: ending it, over the one keeper of the
//! session the composition root makes and hands to the farmer too. A
//! session is kept in memory alone, so there's no data layer.

use std::sync::Arc;

use session::{EndSession, SessionKeeper, end_session};

pub struct SessionComponent {
    pub end: EndSession,
}

impl SessionComponent {
    pub fn new(sessions: Arc<SessionKeeper>) -> Self {
        Self {
            end: end_session(sessions),
        }
    }
}
