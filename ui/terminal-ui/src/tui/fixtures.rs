// The screens' fixtures: an App in each state the spec's mockups show, built
// from the spec's data set (viewmodel::fixtures) over the domain's test
// doubles, on a fixed clock, with the spinner on its first frame. The golden
// tests and the previews render them.

use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use account::test_support::{fixed_account, no_refresh, recording_unlink, refusing_link};
use chrono::{DateTime, TimeDelta, Utc};
use farming::test_support::idle_farmer;
use library::test_support::fixed_library;
use market::{
    Basis, MarketEvent, MarketEventKind, MarketPause, MarketSettings, Wallet,
    test_support::{InMemoryMarketRepository, clock_from, fixed_prices},
};
use preferences::{
    get_preferences, set_appear_online, set_game_tier, set_only_priority,
    test_support::InMemoryPreferencesRepository,
};

use account::LoginChallenge;

use super::{App, Clock, GamesView, LogView, Logged, LoginView, Overlay};
use crate::viewmodel::{
    Account, Farming, Games, Library, LogEntry, LogKind, Login, Market, MarketNews, Onboarding,
    Run, fixtures as data, market_line,
};

/// An App showing `data`, as its screens would once the farmer, the market
/// and the account had said what `data` holds.
pub(crate) fn app(data: data::DataSet) -> App {
    let accounts = fixed_account(data.account.clone());
    let prefs = Arc::new(InMemoryPreferencesRepository::new(data.prefs.clone()));
    let get = get_preferences(prefs.clone());
    let market_repo = Arc::new(InMemoryMarketRepository::new(clock_from(data.now)));
    let basis = data.basis;
    let wallet: Option<Wallet> = data.wallet;
    let market = Market::new(
        fixed_prices(data.prices.clone()),
        market::want_prices(market_repo.clone()),
        Arc::new(|_, _| tokio::spawn(async {})),
        market::refresh_prices(market_repo.clone(), clock_from(data.now)),
        market::price_offers(market_repo.clone(), clock_from(data.now)),
        Arc::new(move || wallet),
        Arc::new(move || MarketSettings { basis }),
        market::set_basis(market_repo),
    );
    let mut app = App::new(
        Account::new(accounts.clone(), no_refresh(), recording_unlink().0),
        Login::new(refusing_link()),
        Farming::new(
            idle_farmer(),
            Arc::new(|| {}),
            accounts.clone(),
            get.clone(),
            set_game_tier(prefs.clone()),
        ),
        Games::new(
            get,
            set_game_tier(prefs.clone()),
            set_only_priority(prefs.clone()),
            set_appear_online(prefs),
        ),
        Library::new(fixed_library(data.status.library.clone())),
        Onboarding::new(accounts),
        market,
    );
    app.clock = Clock::Fixed {
        now: data.now,
        zone: data::zone(),
    };
    match data.run {
        Run::Running => app.farming.start(),
        Run::Paused => app.paused_by_user = true,
        Run::Stopped => {}
    }
    app.forecast = data.forecast.clone();
    app.forecast_for = (data.status.session.drops.len(), data.status.order.clone());
    app.selected = Some(data.selected);
    app.status = Some(data.status);
    app
}

/// A time on the fixed clock's day, to the second.
fn at(h: u32, m: u32, s: u32) -> DateTime<Utc> {
    data::at(h, m) + TimeDelta::seconds(i64::from(s))
}

/// Puts a line in the log as if it came a while ago: long enough that it
/// no longer takes the strip from an alert.
fn log(app: &mut App, at: DateTime<Utc>, kind: LogKind, text: &str) {
    app.log.push(Logged {
        entry: LogEntry {
            at,
            kind,
            text: text.to_owned(),
        },
        received: Instant::now()
            .checked_sub(Duration::from_secs(60))
            .unwrap_or_else(Instant::now),
    });
}

/// The afternoon's log, as the log's mockups show it: 211 events, the
/// newest the market's round of prices at 17:29.
fn afternoon(app: &mut App) {
    // Earlier events, which scroll out of sight: routine looks.
    for i in 0..186 {
        let when = at(9, 14, 0) + TimeDelta::seconds(i64::from(i) * 110);
        log(app, when, LogKind::Progress, "Looked at the cards");
    }
    let shown: [((u32, u32, u32), LogKind, &str); 24] = [
        (
            (14, 55, 27),
            LogKind::Progress,
            "Looked at Celeste's cards: 2 to go",
        ),
        (
            (15, 9, 2),
            LogKind::Progress,
            "Steam says new items arrived",
        ),
        (
            (15, 9, 4),
            LogKind::Dropped,
            "Madeline dropped for Celeste — 1 to go",
        ),
        (
            (15, 24, 19),
            LogKind::Progress,
            "Looked at Celeste's cards: 1 to go",
        ),
        (
            (15, 41, 2),
            LogKind::Progress,
            "Steam says new items arrived",
        ),
        (
            (15, 41, 4),
            LogKind::Dropped,
            "Badeline dropped for Celeste, £0.06",
        ),
        (
            (15, 46, 8),
            LogKind::Dropped,
            "Every card has dropped for Celeste",
        ),
        (
            (15, 46, 10),
            LogKind::Playing,
            "Farming Gorogoa — 2 cards to drop",
        ),
        (
            (16, 1, 25),
            LogKind::Progress,
            "Looked at Gorogoa's cards: 2 to go",
        ),
        (
            (16, 15, 2),
            LogKind::Progress,
            "Steam says new items arrived",
        ),
        (
            (16, 15, 4),
            LogKind::Dropped,
            "The Boy dropped for Gorogoa, £0.04 — 1 to go",
        ),
        (
            (16, 30, 19),
            LogKind::Progress,
            "Looked at Gorogoa's cards: 1 to go",
        ),
        (
            (16, 45, 34),
            LogKind::Progress,
            "Looked at Gorogoa's cards: 1 to go",
        ),
        (
            (16, 48, 1),
            LogKind::Progress,
            "Steam says new items arrived",
        ),
        (
            (16, 48, 3),
            LogKind::Dropped,
            "The Fruit dropped for Gorogoa: no one is selling it",
        ),
        (
            (16, 53, 10),
            LogKind::Dropped,
            "Every card has dropped for Gorogoa",
        ),
        (
            (16, 53, 11),
            LogKind::Playing,
            "Farming Heavy Rain — 3 cards to drop",
        ),
        (
            (17, 5, 2),
            LogKind::Progress,
            "Steam says new items arrived",
        ),
        (
            (17, 5, 4),
            LogKind::Dropped,
            "Madison dropped for Heavy Rain (a 2nd copy), £0.05 — 2 to go",
        ),
        (
            (17, 20, 19),
            LogKind::Progress,
            "Looked at Heavy Rain's cards: 2 to go",
        ),
        (
            (17, 23, 40),
            LogKind::Progress,
            "Steam says new items arrived",
        ),
        (
            (17, 23, 42),
            LogKind::Dropped,
            "A card dropped for Heavy Rain — 1 to go",
        ),
        (
            (17, 23, 42),
            LogKind::Progress,
            "Asking Steam which card it was",
        ),
        (
            (17, 28, 42),
            LogKind::Progress,
            "Looked at Heavy Rain's cards: 1 to go",
        ),
    ];
    for ((h, m, s), kind, text) in shown {
        log(app, at(h, m, s), kind, text);
    }
    // The market's round, written as the log writes the market's events.
    let round = MarketNews {
        event: MarketEvent {
            kind: MarketEventKind::AllPriced {
                games: 57,
                next_round: Some(data::at(21, 21)),
            },
            message: String::new(),
        },
        again: false,
    };
    let line = {
        let known = app.known();
        market_line(&round, &app.snapshot(&known))
    };
    log(app, at(17, 29, 14), line.kind, &line.text);
}

/// Farming Heavy Rain alone at 17:31 (mockup a).
pub(crate) fn farming_alone() -> App {
    let mut app = app(data::farming_alone());
    afternoon(&mut app);
    app
}

/// The queue scrolled to its end, Warframe chosen (mockup a, scrolled).
pub(crate) fn queue_at_its_end() -> App {
    let mut app = app(data::queue_at_its_end());
    afternoon(&mut app);
    app.queue_offset = usize::MAX;
    app
}

/// Building hours with the 12-game group, Stray #1 (mockup b).
pub(crate) fn building_hours() -> App {
    let mut app = app(data::building_hours());
    afternoon(&mut app);
    log(
        &mut app,
        at(17, 24, 0),
        LogKind::MovedOn,
        "Stray is priority #1: building its hours first, with 11 others",
    );
    app
}

/// Three minutes into the session (mockup c).
pub(crate) fn first_minutes() -> App {
    let mut app = app(data::first_minutes());
    log(
        &mut app,
        at(9, 14, 10),
        LogKind::Playing,
        "Farming Hollow Knight — 3 cards to drop",
    );
    app
}

/// Reading the badges for the first time (mockups c, at three sizes).
pub(crate) fn reading_badges() -> App {
    let mut app = app(data::reading_badges());
    app.selected = None;
    log(
        &mut app,
        at(9, 14, 5),
        LogKind::Info,
        "Signed on to Steam as alice: reading the badges",
    );
    app
}

/// Hades played on another device since 17:29 (mockup d).
pub(crate) fn waiting_for_hades() -> App {
    let mut app = app(data::waiting_for_hades());
    afternoon(&mut app);
    log(
        &mut app,
        at(17, 29, 30),
        LogKind::Waiting,
        "Hades started on another device: farming waits for it",
    );
    app
}

/// Paused by the user, a moment ago (mockup f).
pub(crate) fn paused() -> App {
    let mut app = app(data::paused());
    afternoon(&mut app);
    app.flash("Paused: nothing is played until you carry on. Press p.");
    app
}

/// The saved sign-in no longer taken (mockup g).
pub(crate) fn sign_in_expired() -> App {
    let mut app = app(data::sign_in_expired());
    afternoon(&mut app);
    app
}

/// The connection lost at 17:30, tried again in 42 seconds (mockup g).
pub(crate) fn reconnecting() -> App {
    let mut app = app(data::reconnecting());
    afternoon(&mut app);
    app
}

/// Every card dropped, five days on (mockup e).
pub(crate) fn nothing_to_farm() -> App {
    let mut app = app(data::nothing_to_farm());
    app.show_done = true;
    app.estimated = Some(market::Money::new(1_679, market::Currency::GBP));
    log(
        &mut app,
        data::on(5, 17, 44),
        LogKind::Dropped,
        "Every card has dropped for Vampire Survivors — nothing left to farm",
    );
    app
}

/// Steam has paused price lookups since 13:31; the next try is at 17:41
/// (mockup h).
pub(crate) fn prices_paused() -> App {
    let set = data::prices_paused();
    let mut app = app(set);
    afternoon(&mut app);
    // What the watcher said: Steam turned lookups down at 13:31, then each
    // try since, the wait doubling to an hour.
    let steps = [
        (10, (13, 41)),
        (20, (14, 1)),
        (40, (14, 41)),
        (60, (15, 41)),
        (60, (16, 41)),
        (60, (17, 41)),
    ];
    for (minutes, (h, m)) in steps {
        app.market.note(MarketEvent {
            kind: MarketEventKind::Paused(MarketPause {
                until: data::at(h, m),
                step: Duration::from_secs(minutes * 60),
            }),
            message: String::new(),
        });
    }
    let told = LogEntry {
        at: data::at(16, 41),
        kind: LogKind::Waiting,
        text: "Steam turned down price lookups again: they wait until 17:41. Farming carries on."
            .to_owned(),
    };
    app.paused_by = Some(told);
    app
}

/// The App a mockup of the spec shows, by the mockup's title, with its
/// pop-up open; `None` for a mockup of something not built yet (quick-sell).
pub(crate) fn for_mockup(title: &str) -> Option<App> {
    let with = |mut app: App, overlay: Overlay| {
        app.overlay = Some(overlay);
        app
    };
    let log = || {
        Overlay::Log(LogView {
            offset: usize::MAX,
            max: 0,
            follow: true,
        })
    };
    Some(match title {
        "dashboard, farming alone (L)"
        | "dashboard, farming alone, the user's window (L)"
        | "dashboard, farming alone (M, tall)"
        | "dashboard, farming alone (M)"
        | "dashboard, farming alone (S)"
        | "dashboard, farming alone (XS)"
        | "too small" => farming_alone(),
        "the queue scrolled to its end (S)" => queue_at_its_end(),
        "building hours with the 12-game group" => building_hours(),
        "the first minutes of a session" => first_minutes(),
        "reading the badges (M)" | "reading the badges (S)" | "reading the badges (XS)" => {
            reading_badges()
        }
        "Steam played on another device" => waiting_for_hades(),
        "nothing left to farm, with the session's summary" => nothing_to_farm(),
        "paused" => paused(),
        "sign-in expired" => sign_in_expired(),
        "connection lost, retrying" => reconnecting(),
        "prices paused by Steam, some stale" => prices_paused(),
        "the market view, prices paused" => with(prices_paused(), Overlay::Market),
        "game details" | "game details (XS)" => with(farming_alone(), Overlay::Detail),
        "this session's haul" => with(farming_alone(), Overlay::Haul),
        "help" => with(farming_alone(), Overlay::Help),
        "games and settings" => {
            let mut data = data::farming_alone();
            data.prefs.priority_games = vec![data::LIMBO, data::STRAY];
            let mut app = app(data);
            afternoon(&mut app);
            app.flash("Stray is priority #2: farmed first.");
            with(app, Overlay::Games(GamesView { cursor: 1 }))
        }
        "sign in with a QR code, scanned" => with(
            farming_alone(),
            Overlay::Login(LoginView {
                challenge: Some(LoginChallenge {
                    url: "https://s.team/q/1/12539683334892375075".into(),
                    scanned: true,
                }),
                outcome: None,
                finished: None,
                from_account: true,
            }),
        ),
        "account" => with(farming_alone(), Overlay::Account { confirm: false }),
        "the log" | "the log (S)" | "the log (XS)" => with(farming_alone(), log()),
        "onboarding, ready to farm" => {
            // Before farming starts: the library as the session found it,
            // 62 games and 252 drops to farm.
            let mut data = data::first_minutes();
            data.run = Run::Stopped;
            let mut app = app(data);
            app.onboarding.signed_out();
            app.onboarding_next();
            app.onboarding_next();
            app
        }
        "onboarding, welcome (XS)" => {
            let mut data = data::farming_alone();
            data.account = None;
            data.run = Run::Stopped;
            app(data)
        }
        _ => return None,
    })
}

/// The basis the App's market values on.
fn basis(app: &App) -> Basis {
    app.market.basis()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::viewmodel::Activity;

    #[tokio::test]
    async fn each_fixture_is_the_state_its_mockup_shows() {
        let activity = |app: &App| {
            let known = app.known();
            format!("{:?}", Activity::of(&app.snapshot(&known)))
        };
        let cases = [
            (farming_alone(), "Farming"),
            (queue_at_its_end(), "Farming"),
            (building_hours(), "BuildingHours"),
            (first_minutes(), "Farming"),
            (reading_badges(), "Reading"),
            (waiting_for_hades(), "Waiting"),
            (paused(), "Paused"),
            (sign_in_expired(), "Expired"),
            (reconnecting(), "Reconnecting"),
            (nothing_to_farm(), "NothingToFarm"),
            (prices_paused(), "Farming"),
        ];
        for (app, said) in cases {
            assert!(activity(&app).starts_with(said), "{}", activity(&app));
            assert_eq!(basis(&app), Basis::List);
        }
    }

    #[tokio::test]
    async fn every_mockup_but_quick_sells_has_its_fixture() {
        for m in crate::tui::golden::mockups() {
            let later = m.title.starts_with("LATER");
            assert_eq!(for_mockup(&m.title).is_none(), later, "{}", m.title);
        }
    }

    #[tokio::test]
    async fn onboarding_sizes_up_the_job_before_farming_starts() {
        let app = for_mockup("onboarding, ready to farm").unwrap();
        assert_eq!(app.onboarding.step(), Some(crate::viewmodel::Step::Start));
        assert!(!app.farming.is_running());
        let library = app.known_library();
        assert_eq!(
            (library.drops_left(), library.with_drops_left().count()),
            (254, 63)
        );
        let welcome = for_mockup("onboarding, welcome (XS)").unwrap();
        assert_eq!(
            welcome.onboarding.step(),
            Some(crate::viewmodel::Step::Welcome)
        );
    }

    #[tokio::test]
    async fn the_afternoons_log_has_211_events() {
        let app = farming_alone();
        assert_eq!(app.log.len(), 211);
        let last = &app.log.last().unwrap().entry;
        assert_eq!(
            last.text,
            "Prices: all 57 games looked up; the next round is at 21:21"
        );
        let paused = prices_paused();
        assert_eq!(paused.market.paused_since(), Some(data::at(13, 31)));
        assert_eq!(
            paused.market.pause().map(|p| p.until),
            Some(data::at(17, 41))
        );
    }
}
