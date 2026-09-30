// The market: prices in the background, the value basis, the wallet, and
// Steam's pause (docs/design/ui.md §5.3); and the market view (m, mockup h):
// where every price comes from.
//
// Prices go through the market's one queue, so what's asked of it is kept
// to what the screens need: the watcher prices the games in the order they
// matter, a game's set is asked for again when one of its cards drops, and
// order books, the instant basis's, only for the cards held while that basis
// is chosen and the chosen game's cards when its details open.

use std::{collections::HashMap, sync::Arc, time::Duration};

use chrono::{DateTime, FixedOffset, Utc};
use market::{
    Basis, GetMarketSettings, GetPrices, GetWallet, MarketError, MarketEvent, MarketEventKind,
    MarketPause, Money, Price, PriceBook, PriceOffers, RefreshPrices, SetBasis, Wallet, WantPrices,
    WatchPrices, expected_per_drop, value_left, value_of,
};
use tokio::{sync::mpsc, task::JoinHandle};
use tokio_util::sync::CancellationToken;

use super::{
    haul::{Haul, Unpriced},
    screen::{Cell, Snapshot},
};

/// An order book is looked up again once it's half an hour old; asking
/// sooner would only wait in the queue to be told it's fresh.
const OFFERS_AGAIN: Duration = Duration::from_secs(30 * 60);

/// Runs the market's background pricing, and asks it for what the screens
/// need.
pub struct Market {
    prices: GetPrices,
    want: WantPrices,
    watch: WatchPrices,
    refresh: RefreshPrices,
    offers: PriceOffers,
    wallet: GetWallet,
    settings: GetMarketSettings,
    set_basis: SetBasis,
    tx: mpsc::Sender<MarketEvent>,
    events: mpsc::Receiver<MarketEvent>,
    watching: Option<(CancellationToken, JoinHandle<()>)>,
    /// What the watcher was last told to price.
    wanted: Vec<u32>,
    /// Steam's pause on lookups, as the watcher last told of it, and when
    /// Steam first turned one down in it.
    pause: Option<MarketPause>,
    paused_since: Option<DateTime<Utc>>,
    /// Order books asked for, by market hash name, and when.
    offers_asked: HashMap<String, DateTime<Utc>>,
    /// The order books being looked up; one ask at a time.
    offers_pending: Option<JoinHandle<Result<(), MarketError>>>,
}

/// A market event, and whether it tells of a pause again.
#[derive(Debug, Clone, PartialEq)]
pub struct MarketNews {
    pub event: MarketEvent,
    /// A pause told while one was already in force: Steam turned lookups
    /// down again.
    pub again: bool,
}

impl Market {
    #[expect(
        clippy::too_many_arguments,
        reason = "one for each of the market's use cases"
    )]
    pub fn new(
        prices: GetPrices,
        want: WantPrices,
        watch: WatchPrices,
        refresh: RefreshPrices,
        offers: PriceOffers,
        wallet: GetWallet,
        settings: GetMarketSettings,
        set_basis: SetBasis,
    ) -> Self {
        let (tx, events) = mpsc::channel(256);
        Self {
            prices,
            want,
            watch,
            refresh,
            offers,
            wallet,
            settings,
            set_basis,
            tx,
            events,
            watching: None,
            wanted: Vec::new(),
            pause: None,
            paused_since: None,
            offers_asked: HashMap::new(),
            offers_pending: None,
        }
    }

    /// Starts pricing in the background, if it isn't already.
    pub fn start(&mut self) {
        if self.watching.is_some() {
            return;
        }
        let token = CancellationToken::new();
        let task = (self.watch)(token.clone(), self.tx.clone());
        self.watching = Some((token, task));
    }

    /// Stops pricing: it winds down in the background.
    pub fn cancel(&mut self) {
        if let Some((token, _)) = self.watching.take() {
            token.cancel();
        }
    }

    /// Stops pricing, and waits briefly for it to stop.
    pub async fn stop(&mut self) {
        if let Some((token, task)) = self.watching.take() {
            token.cancel();
            let _ = tokio::time::timeout(Duration::from_secs(1), task).await;
        }
    }

    pub fn is_watching(&self) -> bool {
        self.watching.is_some()
    }

    /// Everything priced so far.
    pub fn book(&self) -> Arc<PriceBook> {
        (self.prices)()
    }

    /// The wallet, once Steam has said.
    pub fn wallet(&self) -> Option<Wallet> {
        (self.wallet)()
    }

    pub fn basis(&self) -> Basis {
        (self.settings)().basis
    }

    /// Values money on the next basis, list then net then instant, and
    /// says which that is.
    pub fn next_basis(&self) -> Result<Basis, MarketError> {
        let next = match self.basis() {
            Basis::List => Basis::Net,
            Basis::Net => Basis::Instant,
            Basis::Instant => Basis::List,
        };
        (self.set_basis)(next)?;
        Ok(next)
    }

    /// Tells the watcher which games to price, most urgent first, when that
    /// has changed.
    pub fn want(&mut self, app_ids: Vec<u32>) {
        if app_ids != self.wanted {
            (self.want)(app_ids.clone());
            self.wanted = app_ids;
        }
    }

    /// Prices a game's set again, in the background, if it's over an hour
    /// old: one of its cards dropped. A pause, or a market that couldn't be
    /// asked, the watcher tells of itself.
    pub fn refresh(&self, app_id: u32) {
        drop((self.refresh)(app_id));
    }

    /// Looks up the order books of these cards, those not asked for in the
    /// last half hour, one ask at a time: the instant basis's budget.
    pub fn ask_offers(&mut self, hashes: Vec<String>, now: DateTime<Utc>) {
        if self
            .offers_pending
            .as_ref()
            .is_some_and(|t| !t.is_finished())
        {
            return;
        }
        self.offers_pending = None;
        let due: Vec<String> = hashes
            .into_iter()
            .filter(|h| {
                self.offers_asked
                    .get(h)
                    .is_none_or(|at| (now - *at).to_std().unwrap_or_default() >= OFFERS_AGAIN)
            })
            .collect();
        if due.is_empty() {
            return;
        }
        for h in &due {
            self.offers_asked.insert(h.clone(), now);
        }
        self.offers_pending = Some((self.offers)(due));
    }

    /// The next thing the watcher said, if any, keeping track of Steam's
    /// pause as it goes.
    pub fn try_recv(&mut self) -> Option<MarketNews> {
        let event = self.events.try_recv().ok()?;
        Some(self.note(event))
    }

    /// Takes in what the watcher said: Steam's pause, and when it began.
    pub fn note(&mut self, event: MarketEvent) -> MarketNews {
        let again = matches!(event.kind, MarketEventKind::Paused(_)) && self.pause.is_some();
        match event.kind {
            MarketEventKind::Paused(p) => {
                if self.pause.is_none() {
                    let step = chrono::TimeDelta::from_std(p.step).unwrap_or_default();
                    self.paused_since = Some(p.until - step);
                }
                self.pause = Some(p);
            }
            MarketEventKind::Resumed => {
                self.pause = None;
                self.paused_since = None;
            }
            _ => {}
        }
        MarketNews { event, again }
    }

    /// Steam's pause on price lookups, as the watcher last told of it.
    pub fn pause(&self) -> Option<MarketPause> {
        self.pause
    }

    /// When Steam first turned a lookup down, in the pause going on.
    pub fn paused_since(&self) -> Option<DateTime<Utc>> {
        self.paused_since
    }
}

/// The market view: each game's prices in farm order, then the games with
/// cards this session; which cards this session aren't priced and why; and
/// a banner while Steam has paused lookups.
#[derive(Debug, Clone, PartialEq)]
pub struct MarketView {
    pub basis: Basis,
    pub rows: Vec<MarketRow>,
    /// The games with cards this session, in the order their first card
    /// dropped.
    pub session_rows: Vec<MarketRow>,
    pub unpriced: Vec<Unpriced>,
    pub pause: Option<Banner>,
    pub now: DateTime<Utc>,
    pub zone: FixedOffset,
}

/// A game's prices.
#[derive(Debug, Clone, PartialEq)]
pub struct MarketRow {
    pub app_id: u32,
    pub name: String,
    /// Its normal cards' and foils' prices, lowest to highest, on the basis.
    pub normal: Option<(Money, Money)>,
    pub foil: Option<(Money, Money)>,
    pub per_drop: Option<Cell>,
    pub left: Option<Cell>,
    pub priced: Priced,
}

/// When a game's set was priced.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Priced {
    /// Under 6 hours ago: "2h ago".
    Fresh(Duration),
    /// Over 6 hours ago: "stale 8h", shown dim, and still counted.
    Stale(Duration),
    /// Not looked up yet.
    Pending,
    /// Its lookup failed: tried again then.
    Failed(DateTime<Utc>),
}

/// Steam's pause, as the banner tells it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Banner {
    /// When Steam first turned a lookup down.
    pub since: Option<DateTime<Utc>>,
    /// The next try.
    pub until: DateTime<Utc>,
    /// How long the wait is now.
    pub step: Duration,
}

impl MarketView {
    /// `paused_since` is when Steam first turned a lookup down, in the
    /// pause going on.
    pub fn build(s: &Snapshot<'_>, paused_since: Option<DateTime<Utc>>) -> Self {
        let mut in_session: Vec<u32> = Vec::new();
        for d in s.drops() {
            if !in_session.contains(&d.app_id) {
                in_session.push(d.app_id);
            }
        }
        Self {
            basis: s.basis,
            rows: s.order().iter().map(|&id| row(s, id)).collect(),
            session_rows: in_session.into_iter().map(|id| row(s, id)).collect(),
            unpriced: Haul::build(s).unpriced,
            pause: s.pause.filter(|p| p.until > s.now).map(|p| Banner {
                since: paused_since,
                until: p.until,
                step: p.step,
            }),
            now: s.now,
            zone: s.zone,
        }
    }
}

fn row(s: &Snapshot<'_>, app_id: u32) -> MarketRow {
    let set = s.prices.sets.get(&app_id);
    let game = s.library().game(app_id);
    let range = |foil: bool| {
        let (set, wallet) = (set?, s.wallet?);
        let border = if foil { &set.foil } else { &set.normal };
        let values: Vec<Money> = border
            .iter()
            .filter_map(|c| value_of(&c.price, s.basis.still_to_drop(), &wallet))
            .collect();
        Some((
            *values.iter().min_by_key(|m| m.minor)?,
            *values.iter().max_by_key(|m| m.minor)?,
        ))
    };
    let left_to_drop = game.is_some_and(|g| g.has_drops_left());
    let (per_drop, left) = match (set, s.wallet) {
        _ if !left_to_drop => (None, None),
        (Some(set), Some(wallet)) => match expected_per_drop(set, s.basis, &wallet) {
            Some(value) => (
                Some(Cell::Value { value, stale: None }),
                Some(Cell::Value {
                    value: value_left(s.library(), &[app_id], s.prices, s.basis, &wallet).value,
                    stale: None,
                }),
            ),
            None => (Some(Cell::NoMarket), Some(Cell::NoMarket)),
        },
        _ => (Some(Cell::Pending), Some(Cell::Pending)),
    };
    let quote = set.and_then(|set| {
        set.normal
            .iter()
            .chain(&set.foil)
            .find_map(|c| match &c.price {
                Price::Known(q) => Some(q),
                _ => None,
            })
    });
    let priced = match (set, quote) {
        (None, _) => Priced::Pending,
        (Some(set), _) if set.retry_at.is_some() && quote.is_none() => {
            Priced::Failed(set.retry_at.unwrap_or(set.fetched_at))
        }
        (Some(set), None) => Priced::Fresh((s.now - set.fetched_at).to_std().unwrap_or_default()),
        (Some(_), Some(q)) if q.is_stale(s.now) => Priced::Stale(q.age(s.now)),
        (Some(_), Some(q)) => Priced::Fresh(q.age(s.now)),
    };
    MarketRow {
        app_id,
        name: s.name(app_id),
        normal: range(false),
        foil: range(true),
        per_drop,
        left,
        priced,
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use market::{
        Currency, MarketRepository, MarketSettings, Offers,
        test_support::{InMemoryMarketRepository, clock_from, order_book},
    };

    use super::*;
    use crate::{tui::format, viewmodel::fixtures};

    fn money(c: Option<Cell>) -> String {
        c.and_then(|c| c.value())
            .map(format::money)
            .unwrap_or_default()
    }

    fn range(r: Option<(Money, Money)>) -> String {
        r.map(|(a, b)| {
            let high: String = format::money(b).chars().skip(1).collect();
            format!("{}–{high}", format::money(a))
        })
        .unwrap_or_default()
    }

    #[test]
    fn every_game_in_farm_order_with_its_prices_and_their_age() {
        let data = fixtures::prices_paused();
        let view = MarketView::build(&data.snapshot(), data.paused_since);
        assert_eq!(view.rows.len(), 57);
        let r = |i: usize| {
            let row = &view.rows[i];
            (
                row.name.clone(),
                range(row.normal),
                range(row.foil),
                money(row.per_drop),
                money(row.left),
                row.priced,
            )
        };
        let stale = Priced::Stale(Duration::from_secs(8 * 3600));
        assert_eq!(
            r(0),
            (
                "Heavy Rain".into(),
                "£0.04–0.06".into(),
                "£0.35–0.60".into(),
                "£0.05".into(),
                "£0.05".into(),
                stale
            )
        );
        assert_eq!(
            r(1),
            (
                "LIMBO".into(),
                "£0.04–0.09".into(),
                "£0.49–1.35".into(),
                "£0.06".into(),
                "£0.12".into(),
                stale
            )
        );
        assert_eq!(r(3).1, "£0.07–0.12", "Anno 1800");
        assert_eq!(r(4).1, "£0.03–0.07", "Crypt of the NecroDancer");
        assert_eq!(r(17).4, "£0.08", "TCG Card Shop Simulator");
        assert_eq!(view.rows[0].app_id, fixtures::HEAVY_RAIN);
        assert_eq!(view.basis, Basis::List);
        assert_eq!(view.now, fixtures::now());
    }

    #[test]
    fn the_games_with_cards_this_session_follow() {
        let data = fixtures::farming_alone();
        let view = MarketView::build(&data.snapshot(), None);
        let names: Vec<&str> = view.session_rows.iter().map(|r| r.name.as_str()).collect();
        assert_eq!(
            names,
            [
                "Hollow Knight",
                "Inscryption",
                "Hades",
                "Celeste",
                "Gorogoa",
                "Heavy Rain"
            ]
        );
        assert_eq!(view.session_rows[0].per_drop, None, "nothing left to drop");
        assert_eq!(
            view.session_rows[0].priced,
            Priced::Fresh(Duration::from_secs(4 * 3600 + 29 * 60))
        );
        assert_eq!(view.unpriced.len(), 3);
        assert_eq!(view.pause, None);
    }

    #[test]
    fn the_banner_says_since_when_and_until_when() {
        let data = fixtures::prices_paused();
        let view = MarketView::build(&data.snapshot(), data.paused_since);
        let banner = view.pause.unwrap();
        let zone = fixtures::zone();
        assert_eq!(
            format::clock(banner.since.unwrap(), view.now, zone),
            "13:31"
        );
        assert_eq!(format::clock(banner.until, view.now, zone), "17:41");
        assert_eq!(
            banner.step,
            Duration::from_secs(3600),
            "the wait is an hour now"
        );
    }

    #[test]
    fn a_set_not_looked_up_yet_is_pending() {
        let data = fixtures::first_minutes();
        let view = MarketView::build(&data.snapshot(), None);
        let last = view.rows.last().unwrap();
        assert_eq!(
            (last.priced, last.per_drop, last.normal),
            (Priced::Pending, Some(Cell::Pending), None)
        );
        assert!(matches!(view.rows[0].priced, Priced::Fresh(_)));
    }

    /// A market over an in-memory one, on a clock from the session's start.
    fn market(repo: &Arc<InMemoryMarketRepository>) -> Market {
        let clock = clock_from(fixtures::at(9, 14));
        Market::new(
            market::get_prices(repo.clone()),
            market::want_prices(repo.clone()),
            market::watch_prices(repo.clone(), clock.clone()),
            market::refresh_prices(repo.clone(), clock.clone()),
            market::price_offers(repo.clone(), clock.clone()),
            market::get_wallet(repo.clone()),
            market::get_market_settings(repo.clone()),
            market::set_basis(repo.clone()),
        )
    }

    #[tokio::test(start_paused = true)]
    async fn the_basis_goes_list_net_instant_and_round_again() {
        let repo = Arc::new(InMemoryMarketRepository::new(clock_from(fixtures::at(
            9, 14,
        ))));
        let m = market(&repo);
        assert_eq!(m.basis(), Basis::List);
        assert_eq!(m.next_basis(), Ok(Basis::Net));
        assert_eq!(m.next_basis(), Ok(Basis::Instant));
        assert_eq!(m.next_basis(), Ok(Basis::List));
        assert_eq!(repo.settings(), MarketSettings { basis: Basis::List });
        repo.disk_full();
        assert_eq!(
            m.next_basis(),
            Err(MarketError::Unavailable),
            "that didn't stick"
        );
        assert_eq!(m.basis(), Basis::List);
        assert_eq!(m.wallet().map(|w| w.currency), Some(Currency::GBP));
    }

    #[tokio::test(start_paused = true)]
    async fn prices_are_wanted_only_when_that_changes_and_watched_in_the_background() {
        let repo = Arc::new(InMemoryMarketRepository::new(clock_from(fixtures::at(
            9, 14,
        ))));
        repo.lists(620, &[("Atlas", 8), ("P-Body", 6)], &[]);
        let mut m = market(&repo);
        m.want(vec![620, 440]);
        assert_eq!(repo.wanted(), [620, 440]);
        let sent = Arc::new(Mutex::new(0));
        let counted = sent.clone();
        m.want = Arc::new(move |_| *counted.lock().unwrap() += 1);
        m.want(vec![620, 440]);
        assert_eq!(*sent.lock().unwrap(), 0, "the same list isn't sent again");
        m.want(vec![440]);
        assert_eq!(*sent.lock().unwrap(), 1);

        repo.want(vec![620]);
        m.start();
        m.start();
        assert!(m.is_watching());
        let news = loop {
            if let Some(news) = m.try_recv() {
                break news;
            }
            tokio::time::sleep(Duration::from_secs(1)).await;
        };
        assert!(matches!(
            news.event.kind,
            MarketEventKind::AllPriced { games: 1, .. }
        ));
        assert!(!news.again);
        assert!(m.book().sets.contains_key(&620));
        m.refresh(620);
        m.stop().await;
        assert!(!m.is_watching());
        m.start();
        m.cancel();
        assert!(!m.is_watching(), "signing out stops it");
    }

    #[tokio::test(start_paused = true)]
    async fn a_pause_is_kept_until_steam_answers_again() {
        let repo = Arc::new(InMemoryMarketRepository::new(clock_from(fixtures::at(
            9, 14,
        ))));
        let mut m = market(&repo);
        let pause = |minutes: u64, until: DateTime<Utc>| MarketPause {
            until,
            step: Duration::from_secs(minutes * 60),
        };
        let first = pause(10, fixtures::at(13, 41));
        let second = pause(20, fixtures::at(14, 1));
        for kind in [
            MarketEventKind::Paused(first),
            MarketEventKind::Paused(second),
            MarketEventKind::Resumed,
        ] {
            m.tx.try_send(MarketEvent {
                kind,
                message: String::new(),
            })
            .unwrap();
        }
        let news = m.try_recv().unwrap();
        assert!(!news.again);
        assert_eq!(m.pause(), Some(first));
        assert_eq!(
            m.paused_since(),
            Some(fixtures::at(13, 31)),
            "when Steam turned it down"
        );
        assert!(m.try_recv().unwrap().again, "turned down again");
        assert_eq!(m.pause(), Some(second));
        assert_eq!(m.paused_since(), Some(fixtures::at(13, 31)));
        m.try_recv().unwrap();
        assert_eq!((m.pause(), m.paused_since()), (None, None));
        assert_eq!(m.try_recv(), None);
    }

    #[tokio::test(start_paused = true)]
    async fn order_books_are_asked_for_once_each_half_hour_one_ask_at_a_time() {
        let repo = Arc::new(InMemoryMarketRepository::new(clock_from(fixtures::at(
            9, 14,
        ))));
        repo.offers("960910-Madison", 5, 4);
        let mut m = market(&repo);
        let now = fixtures::at(17, 31);
        m.ask_offers(vec!["960910-Madison".into()], now);
        m.ask_offers(vec!["960910-Madison".into()], now);
        if let Some(task) = m.offers_pending.take() {
            task.await.unwrap().unwrap();
        }
        assert_eq!(repo.offer_lookups(), ["960910-Madison"]);
        m.ask_offers(
            vec!["960910-Madison".into()],
            now + chrono::TimeDelta::minutes(10),
        );
        assert!(
            m.offers_pending.is_none(),
            "asked for under half an hour ago"
        );
        m.ask_offers(
            vec!["960910-Madison".into()],
            now + chrono::TimeDelta::minutes(31),
        );
        assert!(m.offers_pending.is_some(), "and again after it");
        assert_eq!(
            repo.book().offers.get("960910-Madison"),
            Some(&Offers {
                price: order_book(5, 4, repo.book().offers["960910-Madison"].looked_up_at),
                looked_up_at: repo.book().offers["960910-Madison"].looked_up_at,
            })
        );
    }
}
