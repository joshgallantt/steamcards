use std::sync::{Arc, Mutex};

use chrono::{DateTime, Utc};

use crate::KeptSession;

/// Keeps the farming session from one run of the farmer to the next, through
/// pauses, until [`EndSessionUseCase`](crate::EndSessionUseCase) ends it. The farmer and
/// that use case share one; nothing else reaches into it. A session is
/// never kept on disk: quitting steamcards ends it too.
#[derive(Default)]
pub struct SessionKeeper {
    current: Mutex<Option<Arc<Mutex<KeptSession>>>>,
}

impl SessionKeeper {
    /// The session going on, or a new one starting `now`.
    pub fn current(&self, now: DateTime<Utc>) -> Arc<Mutex<KeptSession>> {
        self.current
            .lock()
            .unwrap()
            .get_or_insert_with(|| Arc::new(Mutex::new(KeptSession::new(now))))
            .clone()
    }

    /// Ends the session: the next run starts a new one. A run still going
    /// carries on with the old one, on its own.
    pub fn end(&self) {
        self.current.lock().unwrap().take();
    }
}
