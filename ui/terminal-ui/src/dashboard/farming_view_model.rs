use std::{sync::Arc, time::Duration};

use account::GetAccountUseCase;
use farming::{FarmCardsUseCase, FarmingUpdate};
use preferences::{GetPreferencesUseCase, Preferences, PreferencesError, SetGameTierUseCase, Tier};
use session::EndSessionUseCase;
use steam_library::AppId;
use tokio::{sync::mpsc, task::JoinHandle};
use tokio_util::sync::CancellationToken;

/// Runs the farmer, pauses and resumes it, and ends its session.
pub struct FarmingViewModel {
    farm: Arc<dyn FarmCardsUseCase>,
    end_session: Arc<dyn EndSessionUseCase>,
    account: Arc<dyn GetAccountUseCase>,
    get_preferences: Arc<dyn GetPreferencesUseCase>,
    set_tier: Arc<dyn SetGameTierUseCase>,
    tx: mpsc::Sender<FarmingUpdate>,
    updates: mpsc::Receiver<FarmingUpdate>,
    running: Option<(CancellationToken, JoinHandle<()>)>,
    /// The account this session of farming is for: who was signed in when
    /// it began. `None` until farming first starts, and once it ends.
    session_for: Option<String>,
}

impl FarmingViewModel {
    pub fn new(
        farm: Arc<dyn FarmCardsUseCase>,
        end_session: Arc<dyn EndSessionUseCase>,
        account: Arc<dyn GetAccountUseCase>,
        get_preferences: Arc<dyn GetPreferencesUseCase>,
        set_tier: Arc<dyn SetGameTierUseCase>,
    ) -> Self {
        let (tx, updates) = mpsc::channel(1024);
        Self {
            farm,
            end_session,
            account,
            get_preferences,
            set_tier,
            tx,
            updates,
            running: None,
            session_for: None,
        }
    }

    /// Starts farming, when signed in and not farming already.
    pub fn start(&mut self) {
        let Some(account) = self.account.call() else {
            return;
        };
        if self.running.is_some() {
            return;
        }
        self.session_for.get_or_insert(account.name);
        let token = CancellationToken::new();
        let task = self.farm.call(token.clone(), self.tx.clone());
        self.running = Some((token, task));
    }

    /// Stops farming; the farmer stops playing and signs off by itself.
    pub fn pause(&mut self) {
        if let Some((token, _)) = self.running.take() {
            token.cancel();
        }
    }

    /// Stops farming and waits, briefly, for the farmer to sign off, so
    /// Steam hears it before the app goes.
    pub async fn stop(&mut self) {
        if let Some((token, task)) = self.running.take() {
            token.cancel();
            let _ = tokio::time::timeout(Duration::from_secs(3), task).await;
        }
    }

    pub fn is_running(&self) -> bool {
        self.running.is_some()
    }

    /// Ends this session of farming: farming again starts a new one.
    pub fn end_session(&mut self) {
        self.end_session.call();
        self.session_for = None;
    }

    /// Someone signed in. For the account this session is for, it goes on;
    /// for another, it ends, since a session is one account's: that
    /// account's farming starts afresh.
    pub fn signed_in(&mut self) {
        let Some(farmed_for) = &self.session_for else {
            return;
        };
        let same = self
            .account
            .call()
            .is_some_and(|a| !a.name.is_empty() && a.name == *farmed_for);
        if !same {
            self.end_session();
        }
    }

    pub fn try_recv(&mut self) -> Option<FarmingUpdate> {
        self.updates.try_recv().ok()
    }

    pub fn preferences(&self) -> Preferences {
        self.get_preferences.call()
    }

    /// Takes effect within moments: the farmer looks at the preferences as
    /// it plays.
    pub fn set_tier(&self, app_id: AppId, tier: Tier) -> Result<(), PreferencesError> {
        self.set_tier.call(app_id, tier)
    }
}

#[cfg(test)]
mod tests {
    use account::{Account, test_support::StubGetAccountUseCase};
    use farming::test_support::SpyFarmCardsUseCase;
    use preferences::{
        DefaultSetGameTierUseCase,
        test_support::{FakePreferencesRepository, StubGetPreferencesUseCase},
    };
    use session::test_support::SpyEndSessionUseCase;

    use super::*;

    /// Farming, signed in or not, counting the farmer's runs.
    fn farming(signed_in: bool, farm: &Arc<SpyFarmCardsUseCase>) -> FarmingViewModel {
        let account = signed_in.then(|| Account {
            name: "cardfarmer".into(),
            expired: false,
        });
        FarmingViewModel::new(
            farm.clone(),
            Arc::new(SpyEndSessionUseCase::default()),
            Arc::new(StubGetAccountUseCase::new(account)),
            Arc::new(StubGetPreferencesUseCase::default()),
            Arc::new(DefaultSetGameTierUseCase::new(Arc::new(
                FakePreferencesRepository::default(),
            ))),
        )
    }

    #[tokio::test]
    async fn farming_takes_a_sign_in() {
        let farm = Arc::new(SpyFarmCardsUseCase::default());
        let mut f = farming(false, &farm);
        f.start();
        assert!(!f.is_running());
        assert_eq!(farm.runs(), 0, "the farmer never started");
    }

    /// Farming, signed in as whoever `who` says, counting the sessions
    /// ended.
    fn farming_as(
        who: &Arc<StubGetAccountUseCase>,
        ended: &Arc<SpyEndSessionUseCase>,
    ) -> FarmingViewModel {
        FarmingViewModel::new(
            Arc::new(SpyFarmCardsUseCase::default()),
            ended.clone(),
            who.clone(),
            Arc::new(StubGetPreferencesUseCase::default()),
            Arc::new(DefaultSetGameTierUseCase::new(Arc::new(
                FakePreferencesRepository::default(),
            ))),
        )
    }

    fn account(name: &str) -> Option<Account> {
        Some(Account {
            name: name.into(),
            expired: false,
        })
    }

    #[tokio::test]
    async fn signing_in_as_another_account_ends_the_session() {
        let who = Arc::new(StubGetAccountUseCase::new(account("cardfarmer")));
        let ended = Arc::new(SpyEndSessionUseCase::default());
        let mut f = farming_as(&who, &ended);
        f.start();

        // The same account again, its sign-in renewed: the session goes on.
        f.signed_in();
        assert_eq!(ended.ended(), 0);

        who.set(account("someone_else"));
        f.signed_in();
        assert_eq!(ended.ended(), 1, "another account's farming starts afresh");

        f.pause();
        f.start();
        f.signed_in();
        assert_eq!(ended.ended(), 1, "and is then its own session");
    }

    #[tokio::test]
    async fn pausing_stops_the_farmer_and_starting_runs_it_again() {
        let farm = Arc::new(SpyFarmCardsUseCase::default());
        let mut f = farming(true, &farm);
        f.start();
        f.start();
        assert!(f.is_running());
        assert_eq!(farm.runs(), 1, "once, however often it's asked");
        f.pause();
        assert!(!f.is_running());
        f.start();
        f.stop().await;
        assert!(!f.is_running());
        assert_eq!(farm.runs(), 2);
    }
}
