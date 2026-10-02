use std::sync::Arc;

use preferences::GetPreferencesUseCase;
use tokio::{sync::mpsc, task::JoinHandle};
use tokio_util::sync::CancellationToken;

use crate::{
    KeepUpToDateUseCase, ReleaseRepository, UpdateEvent, UpdatedBy, Version,
    model::rules::LOOK_EVERY,
};

pub struct DefaultKeepUpToDateUseCase {
    repo: Arc<dyn ReleaseRepository>,
    get_preferences: Arc<dyn GetPreferencesUseCase>,
    running: Version,
    updated_by: UpdatedBy,
}

impl DefaultKeepUpToDateUseCase {
    /// `running` is this copy's version, and `updated_by` who updates it.
    pub fn new(
        repo: Arc<dyn ReleaseRepository>,
        get_preferences: Arc<dyn GetPreferencesUseCase>,
        running: Version,
        updated_by: UpdatedBy,
    ) -> Self {
        Self {
            repo,
            get_preferences,
            running,
            updated_by,
        }
    }
}

impl KeepUpToDateUseCase for DefaultKeepUpToDateUseCase {
    fn call(&self, token: CancellationToken, events: mpsc::Sender<UpdateEvent>) -> JoinHandle<()> {
        let repo = Arc::clone(&self.repo);
        let get_preferences = Arc::clone(&self.get_preferences);
        let by = self.updated_by;
        // The newest release put in place, or told of: none older is news.
        let mut newest = self.running;
        tokio::spawn(async move {
            loop {
                // Off, it asks nobody anything.
                if get_preferences.call().auto_update
                    && let Ok(Some(latest)) = repo.latest().await
                    && latest > newest
                {
                    let event = match by {
                        UpdatedBy::Itself => match repo.install(latest).await {
                            Ok(()) => UpdateEvent::Installed(latest),
                            Err(_) => UpdateEvent::Available {
                                version: latest,
                                by,
                            },
                        },
                        UpdatedBy::Homebrew | UpdatedBy::Cargo => UpdateEvent::Available {
                            version: latest,
                            by,
                        },
                    };
                    newest = latest;
                    if events.send(event).await.is_err() {
                        return;
                    }
                }
                tokio::select! {
                    () = token.cancelled() => return,
                    () = tokio::time::sleep(LOOK_EVERY) => {}
                }
            }
        })
    }
}
