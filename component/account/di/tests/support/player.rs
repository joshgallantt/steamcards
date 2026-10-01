//! Someone signing in and out of Steam with steamcards: the account
//! component wired as the composition root wires it, over the real data
//! layer and a real config file in a folder of its own. Only Steam is stood
//! in for, by a stand-in server: there's no real one to sign in to.

use std::{
    fs,
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

use account::{Account, LoginChallenge, SignInError, SignOutError};
use account_di::AccountComponent;
use config_file::{ConfigFile, CredentialStore, Credentials};
use debug_log::DebugLog;
use steam_api::{
    SteamClient,
    test_support::{ACCOUNT, FakeSteam, STEAM_ID, token},
};
use tokio::sync::mpsc;

pub(crate) struct Player {
    pub(crate) steam: FakeSteam,
    config: PathBuf,
    account: AccountComponent,
}

impl Player {
    /// Someone with nobody signed in yet.
    pub(crate) async fn new(name: &str) -> Self {
        let steam = FakeSteam::start().await;
        let dir =
            std::env::temp_dir().join(format!("steamcards-account-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let config = dir.join("config.json");
        let account = component(&steam, &config);
        Self {
            steam,
            config,
            account,
        }
    }

    /// Someone signed in already, the last time steamcards ran.
    pub(crate) async fn signed_in(name: &str) -> Self {
        let player = Self::new(name).await;
        let file = ConfigFile::open(player.config.clone()).unwrap();
        file.save_credentials(Credentials {
            refresh_token: token(STEAM_ID, 4_000_000_000),
            account_name: ACCOUNT.into(),
            steam_id: STEAM_ID,
            login_id: 7,
        })
        .unwrap();
        player.comes_back()
    }

    /// Who the account screen shows as signed in.
    pub(crate) fn sees(&self) -> Option<Account> {
        self.account.get_account.call()
    }

    /// Signs in with the Steam app: the codes shown, and how it went.
    pub(crate) async fn signs_in(&self) -> (Vec<LoginChallenge>, Result<(), SignInError>) {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let outcome = self.account.sign_in.call(tx).await.unwrap();
        let mut seen = Vec::new();
        while let Ok(c) = rx.try_recv() {
            seen.push(c);
        }
        (seen, outcome)
    }

    /// Has the saved sign-in checked, in the background as the account
    /// screen does, and waits until Steam has been asked.
    pub(crate) async fn checks_the_sign_in(&self) {
        let asked = self.steam.logons().len();
        self.account.check_sign_in.call();
        eventually(|| self.steam.logons().len() > asked).await;
    }

    pub(crate) fn signs_out(&self) -> Result<(), SignOutError> {
        self.account.sign_out.call()
    }

    /// Quits steamcards and starts it again.
    pub(crate) fn comes_back(self) -> Self {
        let account = component(&self.steam, &self.config);
        Self { account, ..self }
    }

    /// The disk fills up: nothing more can be written to the config file.
    pub(crate) fn runs_out_of_disk(&self) {
        fs::remove_file(&self.config).unwrap();
        fs::create_dir_all(&self.config).unwrap();
    }
}

fn component(steam: &FakeSteam, config: &Path) -> AccountComponent {
    let file = Arc::new(ConfigFile::open(config.to_path_buf()).unwrap());
    let client = SteamClient::with_endpoints(file, &DebugLog::off(), steam.endpoints());
    AccountComponent::new(Arc::new(client))
}

/// Waits until `check` holds, or two seconds have passed.
pub(crate) async fn eventually(check: impl Fn() -> bool) {
    for _ in 0..200 {
        if check() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    assert!(check(), "never happened");
}
