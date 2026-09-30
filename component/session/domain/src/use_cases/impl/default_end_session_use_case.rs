use std::sync::Arc;

use crate::{EndSessionUseCase, SessionKeeper};

pub struct DefaultEndSessionUseCase {
    sessions: Arc<SessionKeeper>,
}

impl DefaultEndSessionUseCase {
    pub fn new(sessions: Arc<SessionKeeper>) -> Self {
        Self { sessions }
    }
}

impl EndSessionUseCase for DefaultEndSessionUseCase {
    fn call(&self) {
        self.sessions.end();
    }
}
