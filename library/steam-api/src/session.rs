//! The saved Steam sign-in, and everything that talks to Steam as it: the
//! signed-on CM connection and steamcommunity.com. The composition root
//! builds one, and the account screen and the farmer share it, so when Steam
//! rejects the sign-in, both know at once.

use std::{
    collections::HashSet,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use anyhow::{anyhow, bail};
use config_file::{CredentialStore, Credentials};
use debug_log::DebugLog;
use rand::Rng;

use crate::{
    Endpoints,
    auth::{self, Approved},
    badges::{BadgeGame, Seen, read_badge_page, read_game_cards_page, seen_by},
    cm::{Connection, LogOn, Refused},
    community::{Community, WebLogin},
    directory,
    inventory::{self, Described, InventoryItem},
    token,
};

/// Badge pages read at most: 150 games each, so a very large library.
const MAX_PAGES: u32 = 100;
/// A site token with less life left than this is replaced first.
const WEB_TOKEN_MARGIN: i64 = 5 * 60;
/// CM servers tried, best first, before giving up on connecting.
const SERVERS_TRIED: usize = 3;
/// How long to wait before asking again about items Steam didn't describe:
/// it can tell of a new item a moment before it can describe it. ASF waits
/// as long before asking for its inventory again.
const ASK_AGAIN_AFTER: Duration = Duration::from_secs(2);

pub struct Session {
    store: Arc<dyn CredentialStore>,
    rejected: AtomicBool,
    endpoints: Endpoints,
    log: DebugLog,
    http: reqwest::Client,
    community: Community,
    /// The signed-on connection, once there is one.
    live: Mutex<Option<Arc<Connection>>>,
    /// Signing on takes a while: one at a time.
    connecting: tokio::sync::Mutex<()>,
    /// A token for steamcommunity.com, and when it stops working.
    web: Mutex<Option<(String, i64)>>,
}

impl Session {
    pub fn new(store: Arc<dyn CredentialStore>, log: &DebugLog) -> Self {
        Self::with_endpoints(store, log, Endpoints::default())
    }

    pub fn with_endpoints(
        store: Arc<dyn CredentialStore>,
        log: &DebugLog,
        endpoints: Endpoints,
    ) -> Self {
        let log = log.tagged("steam");
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .unwrap_or_default();
        Self {
            community: Community::new(http.clone(), endpoints.community.clone(), log.clone()),
            store,
            rejected: AtomicBool::new(false),
            endpoints,
            log,
            http,
            live: Mutex::default(),
            connecting: tokio::sync::Mutex::default(),
            web: Mutex::default(),
        }
    }

    /// Debug lines about Steam, for everything that talks to it.
    pub fn log(&self) -> &DebugLog {
        &self.log
    }

    /// The saved sign-in, if it's one that can be used.
    pub fn credentials(&self) -> Option<Credentials> {
        self.store.credentials().filter(|c| c.steam_id != 0)
    }

    /// True once Steam has rejected the saved sign-in.
    pub fn is_rejected(&self) -> bool {
        self.rejected.load(Ordering::Relaxed)
    }

    pub fn set_rejected(&self, rejected: bool) {
        self.rejected.store(rejected, Ordering::Relaxed);
    }

    /// Whether the saved sign-in has run out, going by its own date.
    pub fn has_expired(&self) -> bool {
        self.credentials()
            .and_then(|c| token::read(&c.refresh_token))
            .is_some_and(|t| t.expires_at <= now())
    }

    /// A connection to Steam that isn't signed on, for signing in: the first
    /// of the best few servers that answers.
    pub async fn open(&self) -> anyhow::Result<Connection> {
        let servers = servers(&self.endpoints, &self.http).await?;
        let mut last = None;
        for url in servers.iter().take(SERVERS_TRIED) {
            match Connection::connect(url, &self.log).await {
                Ok(conn) => return Ok(conn),
                Err(e) => {
                    self.log.line(&format!("{url}: {e}"));
                    last = Some(e);
                }
            }
        }
        Err(last.unwrap_or_else(|| anyhow!("no Steam server to connect to")))
    }

    /// Keeps a sign-in Steam just approved, in place of any before it, and
    /// clears a rejection. A connection signed on as the old one goes.
    pub async fn save(&self, approved: &Approved) -> anyhow::Result<()> {
        // The same login ID across sign-ins: the same computer, to Steam.
        let login_id = self
            .store
            .credentials()
            .map(|c| c.login_id)
            .filter(|&id| id != 0)
            .unwrap_or_else(|| rand::thread_rng().gen_range(1..=u32::MAX));
        self.store.save_credentials(Credentials {
            refresh_token: approved.refresh_token.clone(),
            account_name: approved.account_name.clone(),
            steam_id: approved.steam_id,
            login_id,
        })?;
        self.set_rejected(false);
        *self.web.lock().unwrap() = token::read(&approved.access_token)
            .map(|t| (approved.access_token.clone(), t.expires_at));
        self.disconnect().await;
        Ok(())
    }

    /// Forgets the saved sign-in. Steam is told to end it too, in the
    /// background: the sign-in is already forgotten here either way.
    pub fn forget(&self) -> anyhow::Result<()> {
        let creds = self.credentials();
        self.store.forget_credentials()?;
        self.set_rejected(false);
        *self.web.lock().unwrap() = None;
        let live = self.live.lock().unwrap().take();
        let Some(creds) = creds else {
            return Ok(());
        };
        let log = self.log.clone();
        let endpoints = self.endpoints.clone();
        let http = self.http.clone();
        tokio::spawn(async move {
            let conn = match live.filter(|c| c.is_signed_on()) {
                Some(conn) => conn,
                None => match sign_on(&endpoints, &http, &log, &creds).await {
                    Ok(conn) => Arc::new(conn),
                    Err(e) => return log.line(&format!("signing out: {e}")),
                },
            };
            if let Err(e) = auth::revoke(&conn, &creds.refresh_token).await {
                log.line(&format!("signing out: {e}"));
            }
            conn.log_off().await;
        });
        Ok(())
    }

    /// The signed-on connection: the one there is, or a new one signed on
    /// with the saved sign-in. When Steam won't take the sign-in, the session
    /// is marked rejected, and nothing tries again until it's replaced.
    pub async fn connection(&self) -> anyhow::Result<Arc<Connection>> {
        let _one_at_a_time = self.connecting.lock().await;
        if let Some(conn) = self.current() {
            return Ok(conn);
        }
        let creds = self
            .credentials()
            .ok_or_else(|| anyhow!("not signed in to Steam"))?;
        if self.is_rejected() {
            bail!("Steam didn't accept the saved sign-in — sign in again");
        }
        let conn = match sign_on(&self.endpoints, &self.http, &self.log, &creds).await {
            Ok(conn) => Arc::new(conn),
            Err(e) => {
                let rejected = e
                    .downcast_ref::<Refused>()
                    .filter(|r| r.eresult.is_sign_in_rejected());
                if let Some(r) = rejected {
                    self.set_rejected(true);
                    bail!(
                        "Steam didn't accept the saved sign-in ({}) — sign in again",
                        r.eresult
                    );
                }
                return Err(anyhow!("couldn't sign on to Steam: {e}"));
            }
        };
        *self.live.lock().unwrap() = Some(conn.clone());
        Ok(conn)
    }

    /// The signed-on connection if there's one, without signing on.
    pub fn current(&self) -> Option<Arc<Connection>> {
        self.live
            .lock()
            .unwrap()
            .clone()
            .filter(|c| c.is_signed_on())
    }

    /// Signs off, if signed on.
    pub async fn disconnect(&self) {
        let live = self.live.lock().unwrap().take();
        if let Some(conn) = live {
            conn.log_off().await;
        }
    }

    /// Every game with trading cards the account has, from its badge pages.
    /// Games the badge pages may be wrong about are checked on their own page.
    pub async fn badges(&self) -> anyhow::Result<Vec<BadgeGame>> {
        let path = |id: u64, page: u32| format!("/profiles/{id}/badges?l=english&p={page}");
        let (first, who) = self.page_as_owner(|id| path(id, 1)).await?;
        let first = read_badge_page(&first);
        let mut games = first.games;
        for page in 2..=first.pages.min(MAX_PAGES) {
            let html = self.community.page(&path(who.steam_id, page), &who).await?;
            games.extend(read_badge_page(&html).games);
        }
        // A game can move to the next page while they're read.
        let mut seen = HashSet::new();
        games.retain(|g| seen.insert(g.app_id));
        for game in games.iter_mut().filter(|g| g.unsure) {
            let own = self.game_cards_as(&who, game.app_id).await;
            match own {
                Ok(Some(checked)) => *game = checked,
                Ok(None) => game.unsure = false,
                Err(e) => self.log.line(&format!("checking {}: {e}", game.app_id)),
            }
        }
        Ok(games)
    }

    /// One game's cards, from its own card page; `None` when the page has no
    /// card drops on it.
    pub async fn game_cards(&self, app_id: u32) -> anyhow::Result<Option<BadgeGame>> {
        let path = |id: u64| format!("/profiles/{id}/gamecards/{app_id}?l=english");
        let (html, _) = self.page_as_owner(path).await?;
        Ok(read_game_cards_page(app_id, &html).game)
    }

    /// The account's community items with these asset IDs, as Steam
    /// describes them over the CM connection: each once, in the order asked.
    /// IDs Steam doesn't know are left out. Signs on first if need be.
    ///
    /// Any not described at first are asked about once more, a moment
    /// later. If that fails, what the first ask found still stands.
    pub async fn describe_items(&self, asset_ids: &[u64]) -> anyhow::Result<Vec<InventoryItem>> {
        let mut seen = HashSet::new();
        let wanted: Vec<u64> = asset_ids
            .iter()
            .copied()
            .filter(|id| seen.insert(*id))
            .collect();
        // An ask naming no items isn't filtered: it's the whole inventory.
        if wanted.is_empty() {
            return Ok(Vec::new());
        }
        let first = self.describe_once(&wanted).await?;
        let mut found = first.items;
        let late: Vec<u64> = wanted
            .iter()
            .copied()
            .filter(|id| !found.contains_key(id))
            .collect();
        if !late.is_empty() {
            self.log.line(&format!(
                "items not described yet: {late:?} (missing: {:?}); asking again",
                first.missing
            ));
            tokio::time::sleep(ASK_AGAIN_AFTER).await;
            match self.describe_once(&late).await {
                Ok(again) => found.extend(again.items),
                Err(e) => self.log.line(&format!("describing {late:?} again: {e}")),
            }
        }
        Ok(wanted.iter().filter_map(|id| found.remove(id)).collect())
    }

    async fn describe_once(&self, asset_ids: &[u64]) -> anyhow::Result<Described> {
        inventory::describe(&*self.connection().await?, asset_ids).await
    }

    async fn game_cards_as(
        &self,
        who: &WebLogin,
        app_id: u32,
    ) -> anyhow::Result<Option<BadgeGame>> {
        let path = format!("/profiles/{}/gamecards/{app_id}?l=english", who.steam_id);
        let html = self.community.page(&path, who).await?;
        Ok(read_game_cards_page(app_id, &html).game)
    }

    /// A page as the account's owner sees it. A page shown signed out means
    /// the site token is no good: a new one is made, once.
    async fn page_as_owner(
        &self,
        path: impl Fn(u64) -> String,
    ) -> anyhow::Result<(String, WebLogin)> {
        for fresh in [false, true] {
            let who = self.web_login(fresh).await?;
            let html = self.community.page(&path(who.steam_id), &who).await?;
            match seen_by(&html) {
                Seen::By(id) if id == who.steam_id => return Ok((html, who)),
                seen => self.log.line(&format!(
                    "steamcommunity.com didn't show the page as the account ({seen:?}); making a new token"
                )),
            }
        }
        bail!("steamcommunity.com wouldn't take the sign-in — it'll be tried again")
    }

    /// Who to fetch pages as: the token at hand while it has life left, or a
    /// new one. When Steam renews the refresh token with it, the new one is
    /// saved, since the old one stops working.
    async fn web_login(&self, fresh: bool) -> anyhow::Result<WebLogin> {
        let creds = self
            .credentials()
            .ok_or_else(|| anyhow!("not signed in to Steam"))?;
        let held = self.web.lock().unwrap().clone();
        if !fresh && let Some((token, _)) = held.filter(|(_, exp)| *exp > now() + WEB_TOKEN_MARGIN)
        {
            return Ok(WebLogin {
                steam_id: creds.steam_id,
                access_token: token,
            });
        }
        let conn = self.connection().await?;
        let (access, renewed) = auth::web_token(&conn, &creds.refresh_token, creds.steam_id)
            .await
            .map_err(|e| anyhow!("couldn't sign in to steamcommunity.com: {e}"))?;
        if let Some(refresh_token) = renewed {
            self.log.line("Steam renewed the sign-in");
            self.store.save_credentials(Credentials {
                refresh_token,
                ..creds.clone()
            })?;
        }
        let expires_at = token::read(&access).map_or(now() + 60 * 60, |t| t.expires_at);
        *self.web.lock().unwrap() = Some((access.clone(), expires_at));
        Ok(WebLogin {
            steam_id: creds.steam_id,
            access_token: access,
        })
    }
}

/// Connects to the best CM server that answers and signs on with `creds`.
async fn sign_on(
    endpoints: &Endpoints,
    http: &reqwest::Client,
    log: &DebugLog,
    creds: &Credentials,
) -> anyhow::Result<Connection> {
    let servers = servers(endpoints, http).await?;
    let mut last = None;
    for url in servers.iter().take(SERVERS_TRIED) {
        let conn = match Connection::connect(url, log).await {
            Ok(conn) => conn,
            Err(e) => {
                last = Some(e);
                continue;
            }
        };
        let signed_on = conn
            .log_on(&LogOn {
                refresh_token: &creds.refresh_token,
                account_name: &creds.account_name,
                steam_id: creds.steam_id,
                login_id: creds.login_id,
            })
            .await;
        match signed_on {
            Ok(()) => return Ok(conn),
            // Another server may be free.
            Err(e)
                if e.downcast_ref::<Refused>()
                    .is_some_and(|r| r.eresult.is_temporary()) =>
            {
                last = Some(e);
            }
            Err(e) => return Err(e),
        }
    }
    Err(last.unwrap_or_else(|| anyhow!("no Steam server to connect to")))
}

/// The CM servers to try, best first: the one `endpoints` names, or Steam's
/// list.
async fn servers(endpoints: &Endpoints, http: &reqwest::Client) -> anyhow::Result<Vec<String>> {
    match &endpoints.cm {
        Some(url) => Ok(vec![url.clone()]),
        None => directory::websocket_servers(http, &endpoints.api).await,
    }
}

fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64)
}
