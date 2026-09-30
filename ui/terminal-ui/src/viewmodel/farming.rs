use std::time::{Duration, Instant};

use account::GetAccount;
use farming::{EndSession, FarmCards, FarmingEvent};
use preferences::{GetPreferences, Preferences, PreferencesError, SetGameTier, Tier};
use tokio::{sync::mpsc, task::JoinHandle};
use tokio_util::sync::CancellationToken;

/// Runs the farmer, pauses and resumes it, and ends its session.
pub struct Farming {
    farm: FarmCards,
    end_session: EndSession,
    account: GetAccount,
    get_preferences: GetPreferences,
    set_tier: SetGameTier,
    tx: mpsc::Sender<FarmingEvent>,
    events: mpsc::Receiver<FarmingEvent>,
    running: Option<(CancellationToken, JoinHandle<()>)>,
    started: Option<Instant>,
}

impl Farming {
    pub fn new(
        farm: FarmCards,
        end_session: EndSession,
        account: GetAccount,
        get_preferences: GetPreferences,
        set_tier: SetGameTier,
    ) -> Self {
        let (tx, events) = mpsc::channel(1024);
        Self {
            farm,
            end_session,
            account,
            get_preferences,
            set_tier,
            tx,
            events,
            running: None,
            started: None,
        }
    }

    /// Starts farming, when signed in and not farming already.
    pub fn start(&mut self) {
        if self.running.is_some() || (self.account)().is_none() {
            return;
        }
        let token = CancellationToken::new();
        let task = (self.farm)(token.clone(), self.tx.clone());
        self.running = Some((token, task));
        self.started = Some(Instant::now());
    }

    /// Stops farming; the farmer stops playing and signs off by itself.
    pub fn pause(&mut self) {
        if let Some((token, _)) = self.running.take() {
            token.cancel();
        }
        self.started = None;
    }

    /// Stops farming and waits, briefly, for the farmer to sign off, so
    /// Steam hears it before the app goes.
    pub async fn stop(&mut self) {
        if let Some((token, task)) = self.running.take() {
            token.cancel();
            let _ = tokio::time::timeout(Duration::from_secs(3), task).await;
        }
        self.started = None;
    }

    pub fn is_running(&self) -> bool {
        self.running.is_some()
    }

    /// Ends this session of farming: farming again starts a new one.
    pub fn end_session(&self) {
        (self.end_session)();
    }

    /// How long farming has been running since it was last started.
    pub fn running_for(&self) -> Option<Duration> {
        self.started.map(|t| t.elapsed())
    }

    pub fn try_recv(&mut self) -> Option<FarmingEvent> {
        self.events.try_recv().ok()
    }

    pub fn preferences(&self) -> Preferences {
        (self.get_preferences)()
    }

    /// Takes effect within moments: the farmer looks at the preferences as
    /// it plays.
    pub fn set_tier(&self, app_id: u32, tier: Tier) -> Result<(), PreferencesError> {
        (self.set_tier)(app_id, tier)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use account::{Account, test_support::fixed_account};
    use farming::test_support::idle_farmer;
    use preferences::{
        set_game_tier,
        test_support::{ChangingPreferences, InMemoryPreferencesRepository},
    };

    use super::*;

    fn farming(signed_in: bool) -> Farming {
        let account = signed_in.then(|| Account {
            name: "cardfarmer".into(),
            expired: false,
        });
        Farming::new(
            idle_farmer(),
            Arc::new(|| {}),
            fixed_account(account),
            ChangingPreferences::default().get(),
            set_game_tier(Arc::new(InMemoryPreferencesRepository::default())),
        )
    }

    #[tokio::test]
    async fn farming_takes_a_sign_in() {
        let mut f = farming(false);
        f.start();
        assert!(!f.is_running());
    }

    #[tokio::test]
    async fn pausing_stops_the_clock() {
        let mut f = farming(true);
        f.start();
        assert!(f.is_running() && f.running_for().is_some());
        f.pause();
        assert!(!f.is_running() && f.running_for().is_none());
        f.start();
        f.stop().await;
        assert!(!f.is_running());
    }
}
