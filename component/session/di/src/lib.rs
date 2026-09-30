//! Where the session is wired: ending it, over the one keeper of the
//! session the composition root makes and hands to the farmer too. A
//! session is kept in memory alone, so there's no data layer.

use std::sync::Arc;

use session::{DefaultEndSessionUseCase, EndSessionUseCase, SessionKeeper};

pub struct SessionComponent {
    pub end_session: Arc<dyn EndSessionUseCase>,
}

impl SessionComponent {
    pub fn new(sessions: Arc<SessionKeeper>) -> Self {
        Self {
            end_session: Arc::new(DefaultEndSessionUseCase::new(sessions)),
        }
    }
}
