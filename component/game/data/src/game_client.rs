use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, Mutex},
};

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use game::{AppId, CardDrops, Game, REFUND_WITHIN};
use steam_api::{SteamClient, badges::BadgeGame};

use crate::badge_page::read_badge_page;

/// Badge pages read at most: 150 games each, so a very large library.
const MAX_PAGES: u32 = 100;

/// Steam's say on the account's games.
#[async_trait]
pub trait GameClient: Send + Sync {
    /// Every game with trading cards on the account, each marked private
    /// or not, and with when it was bought, if lately.
    async fn games(&self) -> anyhow::Result<Vec<Game>>;

    /// Whether Steam signed the last session off for another in its place.
    fn replaced(&self) -> bool;
}

/// The account's badge pages, read signed in, as the Steam client fetches
/// them; and what the CM connection says of the games on them: which are
/// private, and which were bought lately.
pub struct SteamGameClient {
    steam: Arc<SteamClient>,
    /// The apps each package holds, by package ID, as Steam's product info
    /// said: each package is asked about once a run.
    package_apps: Mutex<HashMap<u32, Vec<u32>>>,
}

impl SteamGameClient {
    pub fn new(steam: Arc<SteamClient>) -> Self {
        Self {
            steam,
            package_apps: Mutex::default(),
        }
    }

    /// The badge pages' games: every page, in order. Games the badge pages
    /// may be wrong about are checked on their own card page.
    async fn badges(&self) -> anyhow::Result<Vec<Game>> {
        let path = |id: u64, page: u32| format!("/profiles/{id}/badges?l=english&p={page}");
        let (first, who) = self.steam.page_as_owner(|id| path(id, 1)).await?;
        let first = read_badge_page(&first);
        let mut badges = first.games;
        for page in 2..=first.pages.min(MAX_PAGES) {
            let html = self.steam.page(&path(who.steam_id(), page), &who).await?;
            badges.extend(read_badge_page(&html).games);
        }
        // A game can move to the next page while they're read.
        let mut seen = HashSet::new();
        badges.retain(|b| seen.insert(b.app_id));
        let mut games = Vec::with_capacity(badges.len());
        for badge in badges {
            if !badge.unsure {
                games.push(badge.into_domain());
                continue;
            }
            match self.steam.game_cards(badge.app_id).await {
                Ok(Some(checked)) => games.push(card_page_game(checked)),
                Ok(None) => games.push(badge.into_domain()),
                Err(e) => {
                    self.steam
                        .log()
                        .line(&format!("checking {}: {e}", badge.app_id));
                    games.push(badge.into_domain());
                }
            }
        }
        Ok(games)
    }

    /// When each app was last bought, by app ID, of the purchases Steam may
    /// still refund at `now`: those made within [`REFUND_WITHIN`]. Which
    /// apps a purchase was for comes from Steam's product info on its
    /// package.
    async fn bought_lately(
        &self,
        now: DateTime<Utc>,
    ) -> anyhow::Result<HashMap<u32, DateTime<Utc>>> {
        let lately: Vec<_> = self
            .steam
            .licences()
            .await?
            .into_iter()
            .filter(|l| l.is_purchase())
            .filter_map(|l| {
                let at = DateTime::from_timestamp(i64::from(l.got_at), 0)?;
                (now - at < REFUND_WITHIN).then_some((l, at))
            })
            .collect();
        let unasked: Vec<(u32, u64)> = {
            let known = self.package_apps.lock().unwrap();
            lately
                .iter()
                .filter(|(l, _)| !known.contains_key(&l.package_id))
                .map(|(l, _)| (l.package_id, l.access_token))
                .collect()
        };
        if !unasked.is_empty() {
            let mut told = self.steam.package_apps(&unasked).await?;
            let mut known = self.package_apps.lock().unwrap();
            // A package Steam said nothing of holds nothing to skip.
            for (package_id, _) in unasked {
                known.insert(package_id, told.remove(&package_id).unwrap_or_default());
            }
        }
        let known = self.package_apps.lock().unwrap();
        let mut bought: HashMap<u32, DateTime<Utc>> = HashMap::new();
        for (l, at) in &lately {
            for &app in known.get(&l.package_id).into_iter().flatten() {
                let last = bought.entry(app).or_insert(*at);
                *last = (*last).max(*at);
            }
        }
        Ok(bought)
    }
}

#[async_trait]
impl GameClient for SteamGameClient {
    /// The badge pages' games, with what Steam says of them over the CM
    /// connection. When Steam doesn't say which are private, or which were
    /// bought lately, none are taken to be: it's asked again at the next
    /// read.
    async fn games(&self) -> anyhow::Result<Vec<Game>> {
        let mut games = self.badges().await?;
        let log = |what: &str, e: anyhow::Error| self.steam.log().line(&format!("{what}: {e}"));
        let private: HashSet<u32> = match self.steam.private_apps().await {
            Ok(apps) => apps.into_iter().collect(),
            Err(e) => {
                log("which games are private", e);
                HashSet::new()
            }
        };
        let bought = match self.bought_lately(Utc::now()).await {
            Ok(bought) => bought,
            Err(e) => {
                log("which games were bought lately", e);
                HashMap::new()
            }
        };
        for game in &mut games {
            game.private = private.contains(&game.app_id.0);
            game.bought_at = bought.get(&game.app_id.0).copied();
        }
        Ok(games)
    }

    fn replaced(&self) -> bool {
        self.steam.replaced()
    }
}

/// A game as its own card page shows it.
fn card_page_game(b: BadgeGame) -> Game {
    Game {
        app_id: AppId(b.app_id),
        name: b.name,
        hours: b.hours,
        drops: CardDrops {
            received: b.cards_received,
            remaining: b.cards_left,
        },
        badge_level: b.badge_level,
        private: false,
        bought_at: None,
    }
}
