// Renders every screen and state into an in-memory terminal with fake data, so
// layouts can be checked (and eyeballed) without touching a real account:
//
//   cargo test -p terminal-ui previews -- --nocapture
//
// With PREVIEW_DUMP=<dir>, each screen's cells (symbol, colours, modifiers)
// are also written there as JSON, for turning into the README's images
// (`cargo xtask screenshots`).

use std::sync::Arc;

use account::{
    Account as SignedIn, LoginChallenge,
    test_support::{
        SpyCheckSignInUseCase, SpySignOutUseCase, StubGetAccountUseCase, StubGetWalletUseCase,
        StubSignInUseCase,
    },
};
use card::{
    AssetId, Card, CardAsset, CardKind, CardSet, CardSets, PriceBook,
    test_support::{
        SpyKeepCardPricesUpToDateUseCase, SpyRefreshCardPricesUseCase, SpySetCardsToPriceUseCase,
        StubGetCardPricesUseCase, pounds, set_prices,
    },
};
use chrono::{DateTime, FixedOffset, Offset, TimeZone, Utc};
use farming::{FarmingStatus, NothingToFarm, Status, test_support::SpyFarmCardsUseCase};
use game::{AppId, CardDrops, Game, SteamLibrary, test_support::SpyGetLibraryUseCase};
use preferences::{
    DefaultGetPreferencesUseCase, DefaultSetAppearOnlineUseCase, DefaultSetAutoUpdateUseCase,
    DefaultSetGameTierUseCase, DefaultSetOnlyPriorityUseCase, DefaultSetRestartGamesUseCase,
    Preferences, test_support::FakePreferencesRepository,
};
use ratatui::{Terminal, backend::TestBackend, buffer::Buffer, style::Modifier};
use session::{Drop, DropCard, Mode, Session, Stretch, test_support::SpyEndSessionUseCase};
use update::test_support::SpyKeepUpToDateUseCase;

use super::{App, Clock, LogView, Overlay, SignInView};
use crate::{
    account::{AccountViewModel, UpdateViewModel},
    dashboard::{EventKind, FarmingViewModel, LibraryViewModel, MarketViewModel},
    games::GamesViewModel,
    onboarding::{OnboardingViewModel, Step},
    sign_in::SignInViewModel,
};

/// The previews' clock: a fixed time, shown in UTC, so every screen is the
/// same on any machine, on any day.
fn now() -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 9, 29, 14, 5, 0).unwrap()
}

fn utc() -> FixedOffset {
    Utc.fix()
}

const CLOCK: Clock = Clock { now, zone: utc };

fn cardfarmer() -> Option<SignedIn> {
    Some(SignedIn {
        name: "cardfarmer".into(),
        expired: false,
    })
}

fn game(app_id: u32, name: &str, hours: f64, received: u32, remaining: u32) -> Game {
    Game {
        app_id: AppId(app_id),
        name: name.into(),
        hours,
        drops: CardDrops {
            received,
            remaining,
        },
        badge_level: 0,
    }
}

/// A library like a real one: some games dropping, some building hours, some
/// done.
fn library() -> SteamLibrary {
    let mut portal = game(620, "Portal 2", 5.2, 1, 3);
    portal.badge_level = 1;
    SteamLibrary::new(vec![
        portal,
        game(1_145_360, "Hades", 12.5, 3, 1),
        game(413_150, "Stardew Valley", 2.1, 0, 4),
        game(1_086_940, "Baldur's Gate 3", 1.4, 0, 3),
        game(292_030, "The Witcher 3: Wild Hunt", 0.3, 0, 4),
        game(730, "Counter-Strike 2", 350.1, 0, 2),
        game(220, "Half-Life 2", 30.0, 3, 0),
        game(105_600, "Terraria", 42.0, 4, 0),
    ])
}

/// The sets looked at: Portal 2's.
fn sets() -> CardSets {
    let mut sets = CardSets::default();
    sets.update(
        AppId(620),
        CardSet::new(
            CardKind::Normal,
            vec![
                card("Atlas", 2),
                card("P-Body", 1),
                card("Wheatley", 0),
                card("GLaDOS", 0),
                card("Chell", 1),
                card("Space Core", 0),
                card("Cave Johnson", 0),
                card("Turret", 0),
            ],
        ),
    );
    sets
}

fn card(name: &str, owned: u32) -> Card {
    Card {
        name: name.into(),
        owned,
    }
}

fn at(h: u32, m: u32) -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 9, 29, h, m, 0).unwrap()
}

/// What the market says the cards are worth: every set but Baldur's Gate 3's,
/// still on its way.
fn prices() -> PriceBook {
    let mut book = PriceBook::default();
    for set in [
        set_prices(
            620,
            &[
                ("Atlas", 6),
                ("P-Body", 5),
                ("Wheatley", 7),
                ("GLaDOS", 9),
                ("Chell", 6),
                ("Space Core", 5),
                ("Cave Johnson", 8),
                ("Turret", 4),
            ],
            &[("Atlas", 45)],
            at(13, 0),
        ),
        set_prices(
            1_145_360,
            &[("Zagreus", 8), ("Megaera", 7), ("Thanatos", 9), ("Nyx", 9)],
            &[("Thanatos", 62)],
            at(13, 0),
        ),
        set_prices(
            413_150,
            &[("Abigail", 12), ("Sebastian", 14)],
            &[],
            at(13, 0),
        ),
        set_prices(292_030, &[("Geralt", 9), ("Ciri", 11)], &[], at(13, 0)),
    ] {
        book.sets.insert(set.app_id, set);
    }
    book
}

fn hades_card(asset_id: u64, name: &str, kind: CardKind) -> DropCard {
    let border = match kind {
        CardKind::Normal => "",
        CardKind::Foil => " (Foil)",
    };
    DropCard::Identified(CardAsset {
        asset_id: AssetId(asset_id),
        app_id: AppId(1_145_360),
        name: name.into(),
        market_hash_name: format!("1145360-{name}{border}"),
        kind,
        marketable: true,
        tradable: true,
    })
}

/// This session: Hades' last cards, a second Zagreus among them and a foil,
/// then Portal 2's: one named by its card page, one still being found out.
fn session() -> Session {
    let drop = |at, app_id: u32, card, copy| Drop {
        at,
        app_id: AppId(app_id),
        card,
        copy,
    };
    Session {
        started_at: at(12, 30),
        drops: vec![
            drop(
                at(13, 2),
                1_145_360,
                hades_card(11, "Zagreus", CardKind::Normal),
                Some(1),
            ),
            drop(
                at(13, 31),
                1_145_360,
                hades_card(12, "Zagreus", CardKind::Normal),
                Some(2),
            ),
            drop(
                at(13, 58),
                1_145_360,
                hades_card(13, "Thanatos", CardKind::Foil),
                Some(1),
            ),
            drop(
                at(14, 1),
                620,
                DropCard::NameOnly {
                    name: "Atlas".into(),
                },
                Some(2),
            ),
            drop(at(14, 4), 620, DropCard::Identifying, None),
        ],
        stretches: vec![
            Stretch {
                app_ids: vec![AppId(1_145_360)],
                mode: Mode::Cards,
                from: at(12, 30),
                to: Some(at(13, 58)),
            },
            Stretch {
                app_ids: vec![AppId(620)],
                mode: Mode::Cards,
                from: at(13, 58),
                to: None,
            },
        ],
        ..Default::default()
    }
}

fn prefs() -> Preferences {
    Preferences {
        priority_games: vec![AppId(620), AppId(413_150)],
        skipped_games: vec![AppId(730)],
        ..Default::default()
    }
}

fn app(account: Option<SignedIn>, prefs: Preferences) -> App {
    let repo = Arc::new(FakePreferencesRepository::new(prefs));
    let get = Arc::new(DefaultGetPreferencesUseCase::new(repo.clone()));
    let accounts = Arc::new(StubGetAccountUseCase::new(account));
    let mut app = App::new(
        AccountViewModel::new(
            accounts.clone(),
            Arc::new(SpyCheckSignInUseCase::default()),
            Arc::new(SpySignOutUseCase::default()),
        ),
        SignInViewModel::new(Arc::new(StubSignInUseCase)),
        FarmingViewModel::new(
            Arc::new(SpyFarmCardsUseCase::default()),
            Arc::new(SpyEndSessionUseCase::default()),
            accounts.clone(),
            get.clone(),
            Arc::new(DefaultSetGameTierUseCase::new(repo.clone())),
        ),
        GamesViewModel::new(
            get,
            Arc::new(DefaultSetGameTierUseCase::new(repo.clone())),
            Arc::new(DefaultSetOnlyPriorityUseCase::new(repo.clone())),
            Arc::new(DefaultSetAppearOnlineUseCase::new(repo.clone())),
            Arc::new(DefaultSetRestartGamesUseCase::new(repo.clone())),
        ),
        LibraryViewModel::new(Arc::new(SpyGetLibraryUseCase::answering(Ok(library())))),
        OnboardingViewModel::new(accounts),
        MarketViewModel::new(
            Arc::new(StubGetCardPricesUseCase::new(prices())),
            Arc::new(SpySetCardsToPriceUseCase::default()),
            Arc::new(SpyKeepCardPricesUpToDateUseCase::default()),
            Arc::new(SpyRefreshCardPricesUseCase::default()),
            Arc::new(StubGetWalletUseCase::new(Some(pounds()))),
        ),
        UpdateViewModel::new(
            Arc::new(SpyKeepUpToDateUseCase::default()),
            Arc::new(DefaultGetPreferencesUseCase::new(repo.clone())),
            Arc::new(DefaultSetAutoUpdateUseCase::new(repo)),
        ),
    );
    app.clock = CLOCK;
    app
}

/// Farming Portal 2 on its own, with a card dropped and looked at again in
/// 12 minutes.
fn farming_portal() -> FarmingStatus {
    FarmingStatus {
        status: Status::Farming,
        library: library(),
        sets: sets(),
        order: vec![
            AppId(620),
            AppId(413_150),
            AppId(1_145_360),
            AppId(1_086_940),
            AppId(292_030),
        ],
        playing: vec![AppId(620)],
        mode: Some(Mode::Cards),
        blocked_by: None,
        next_look: Some(now() + chrono::Duration::minutes(12)),
        session: session(),
        ..Default::default()
    }
}

fn farming_app() -> App {
    let mut a = app(cardfarmer(), prefs());
    a.farming.start();
    a.status = Some(farming_portal());
    a.push_log(
        EventKind::Playing,
        "Farming Portal 2 — 3 cards to drop".into(),
    );
    a.push_log(
        EventKind::Dropped,
        "A card dropped for Portal 2 — 3 to go".into(),
    );
    a
}

fn render(app: &mut App, w: u16, h: u16) -> Buffer {
    let mut t = Terminal::new(TestBackend::new(w, h)).unwrap();
    app.sync_selection();
    t.draw(|f| app.draw(f)).unwrap();
    t.backend().buffer().clone()
}

/// Prints the screen as text, and with PREVIEW_DUMP set writes its cells as
/// JSON; returns the text for assertions.
#[expect(
    clippy::disallowed_methods,
    reason = "PREVIEW_DUMP switches a test harness, not the app"
)]
fn show(name: &str, buf: Buffer) -> String {
    let area = buf.area;
    let mut text = String::new();
    for y in 0..area.height {
        let row: String = (0..area.width)
            .map(|x| buf[(x, y)].symbol().to_string())
            .collect();
        text.push_str(row.trim_end());
        text.push('\n');
    }
    println!("\n━━━━ {name} ━━━━\n{text}");

    if let Ok(dir) = std::env::var("PREVIEW_DUMP") {
        let cells: Vec<Vec<serde_json::Value>> = (0..area.height)
            .map(|y| {
                (0..area.width)
                    .map(|x| {
                        let c = &buf[(x, y)];
                        serde_json::json!([
                            c.symbol(),
                            format!("{:?}", c.fg),
                            format!("{:?}", c.bg),
                            c.modifier.contains(Modifier::BOLD),
                            c.modifier.contains(Modifier::REVERSED),
                            c.modifier.contains(Modifier::DIM),
                        ])
                    })
                    .collect()
            })
            .collect();
        let slug: String = name
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
            .collect();
        let json = serde_json::json!({ "name": name, "width": area.width, "height": area.height, "cells": cells });
        std::fs::write(format!("{dir}/{slug}.json"), json.to_string()).unwrap();
    }
    text
}

#[tokio::test]
async fn previews() {
    let mut a = farming_app();
    a.selected = Some(AppId(620));
    let big = show(
        "dashboard, the user's window 209×49",
        render(&mut a, 209, 49),
    );
    assert!(big.contains("farming Portal 2 · next check in 12m"));
    assert!(big.contains("cardfarmer · appears offline"));
    assert!(
        big.contains("This session  1h 35m · 5 cards · £0.84+"),
        "how long it has played, and every copy counted"
    );
    assert!(big.contains("To go  15 cards · 5 games · about 8 hours"));
    assert!(big.contains("All games") && big.contains("━") && big.contains("11 of 28"));
    assert!(big.contains("when done ≈ £2.03+"));
    assert!(big.contains("GAME") && big.contains("CARDS") && big.contains("TO COME"));
    assert!(big.contains("#1  Portal 2") && big.contains("1/4"));
    assert!(big.contains("Zagreus, 2nd"), "a spare says so");
    assert!(
        big.contains("★ Thanatos") && big.contains("£0.62"),
        "a foil, at its own price"
    );
    assert!(big.contains("which card?"), "a card still being found out");
    assert!(!big.contains("█"), "no block bars, in any row");

    let mid = show("dashboard 120×30", render(&mut a, 120, 30));
    assert!(mid.contains("This session") && mid.contains("Games"));
    let narrow = show("dashboard 80×24", render(&mut a, 80, 24));
    assert!(narrow.contains("enter  details"));
    let small = show("dashboard 60×16", render(&mut a, 60, 16));
    assert!(small.contains("Portal 2"));

    a.show_done = true;
    let done = show("done games shown 120×30", render(&mut a, 120, 30));
    assert!(done.contains("✓") && done.contains("Terraria"));
    a.show_done = false;

    a.flash("Portal 2 is now priority #1.");
    show("flash message 120×30", render(&mut a, 120, 30));

    a.overlay = Some(Overlay::Detail { scroll: 0 });
    let detail = show("details pop-up 120×30", render(&mut a, 120, 30));
    assert!(detail.contains("Farming now") && detail.contains("Set     3 of 8 cards · 1 spare"));
    assert!(detail.contains("Atlas") && detail.contains("×2") && detail.contains("£0.06"));
    assert!(
        detail.contains("Wheatley") && detail.contains("—"),
        "none of it: a dash, not a tick"
    );
    assert!(
        detail.contains("Portal 2 is now priority #1."),
        "a pop-up doesn't hide what a key just did"
    );
    let short = show("details pop-up 60×16", render(&mut a, 60, 16));
    assert!(short.contains("more ↓"), "what doesn't fit says so");
    a.overlay = Some(Overlay::Detail { scroll: 99 });
    let end = show(
        "details pop-up, scrolled to its end 60×16",
        render(&mut a, 60, 16),
    );
    assert!(end.contains("Farm priority") && !end.contains("more ↓"));
    a.flash = None;
    a.overlay = None;

    // Building hours: several games together.
    let mut b = farming_app();
    b.status = Some(FarmingStatus {
        playing: vec![AppId(413_150), AppId(1_086_940), AppId(292_030)],
        mode: Some(Mode::Hours),
        next_look: Some(now() + chrono::Duration::minutes(54)),
        ..farming_portal()
    });
    b.selected = Some(AppId(1_086_940));
    let hours = show("dashboard, building hours 120×30", render(&mut b, 120, 30));
    assert!(hours.contains("building hours on 3 games · ready in 54m"));
    assert!(hours.contains("▷"));

    // Another device playing.
    let mut blocked = farming_app();
    blocked.status = Some(FarmingStatus {
        status: Status::Blocked,
        playing: Vec::new(),
        mode: None,
        blocked_by: Some(AppId(730)),
        next_look: None,
        ..farming_portal()
    });
    let text = show("playing elsewhere 120×30", render(&mut blocked, 120, 30));
    assert!(text.contains("waiting · Counter-Strike 2 is being played on another device"));

    // It stopped: a minute's grace.
    let mut grace = farming_app();
    grace.status = Some(FarmingStatus {
        status: Status::Blocked,
        playing: Vec::new(),
        mode: None,
        next_look: Some(now() + chrono::Duration::seconds(60)),
        ..farming_portal()
    });
    let text = show(
        "playing elsewhere stopped 120×30",
        render(&mut grace, 120, 30),
    );
    assert!(text.contains("waiting · farming again in 1m"));

    // Nothing left.
    let mut idle = farming_app();
    idle.status = Some(FarmingStatus {
        status: Status::Idle,
        order: Vec::new(),
        playing: Vec::new(),
        mode: None,
        next_look: Some(now() + chrono::Duration::hours(8)),
        nothing_to_farm: Some(NothingToFarm::AllDropped),
        ..farming_portal()
    });
    let text = show("nothing to farm 120×30", render(&mut idle, 120, 30));
    assert!(text.contains("nothing to farm · looks again in 8h"));

    // Reading the badges.
    let mut reading = app(cardfarmer(), prefs());
    reading.farming.start();
    let text = show("reading the badges 120×30", render(&mut reading, 120, 30));
    assert!(text.contains("Reading your badges"));

    // Paused.
    let mut paused = farming_app();
    paused.farming.pause();
    paused.paused_by_user = true;
    let text = show("paused 120×30", render(&mut paused, 120, 30));
    assert!(text.contains("‖ paused · press p to carry on"));

    // Steam rejected the sign-in.
    let mut expired = app(
        Some(SignedIn {
            name: "cardfarmer".into(),
            expired: true,
        }),
        prefs(),
    );
    let text = show("sign-in expired 120×30", render(&mut expired, 120, 30));
    assert!(text.contains("sign-in expired · press a to sign in again"));

    // Pop-ups.
    a.overlay = Some(Overlay::Account { confirm: false });
    let account = show("account 120×30", render(&mut a, 120, 30));
    assert!(account.contains("● cardfarmer") && account.contains("Appear offline while farming"));
    assert!(
        account.contains("them just the same."),
        "wrapped, not cut off"
    );
    a.overlay = Some(Overlay::Account { confirm: true });
    let text = show("account, signing out 120×30", render(&mut a, 120, 30));
    assert!(text.contains("Sign out of Steam"));

    a.overlay = Some(login(
        Some("https://s.team/q/1/12539683334892375075"),
        false,
    ));
    let qr = show("sign-in QR 120×32", render(&mut a, 120, 32));
    assert!(qr.contains("Sign in with the Steam app") && qr.contains("Scan"));
    assert!(
        qr.contains("No password is typed in here."),
        "whole, not cut off"
    );
    assert!(qr.contains("█"), "a QR code");
    a.overlay = Some(login(Some("https://s.team/q/1/12539683334892375075"), true));
    let scanned = show("sign-in QR, scanned 120×32", render(&mut a, 120, 32));
    assert!(scanned.contains("waiting for you to approve"));
    let small = show("sign-in, no room for QR 80×20", render(&mut a, 80, 20));
    assert!(small.contains("Make the window bigger"));
    a.overlay = Some(Overlay::SignIn(SignInView {
        challenge: None,
        outcome: Some(Err(
            "the sign-in wasn't approved in the Steam app — try again".into(),
        )),
        finished: None,
        from_account: true,
    }));
    let failed = show("sign-in failed 120×30", render(&mut a, 120, 30));
    assert!(failed.contains("Couldn't sign in") && failed.contains("try again"));

    a.library.refresh();
    settle(&mut a).await;
    a.overlay = Some(a.open_games());
    let games = show("games 120×30", render(&mut a, 120, 30));
    assert!(games.contains("PRIORITY") && games.contains("OTHER GAMES WITH CARDS TO DROP"));
    assert!(games.contains("#1  Portal 2"));

    a.overlay = Some(Overlay::Log(LogView {
        offset: usize::MAX,
        max: 0,
        follow: true,
    }));
    let log = show("log 120×24", render(&mut a, 120, 24));
    assert!(log.contains("2 events"));

    a.overlay = Some(Overlay::Help);
    let help = show("help 120×40", render(&mut a, 120, 40));
    assert!(help.contains("What gets farmed") && help.contains("rank the selected game"));
    assert!(help.contains("some cards aren't priced yet"));
    show("help, short window 130×24", render(&mut a, 130, 24));

    a.overlay = Some(Overlay::ConfirmQuit);
    let quit = show("quit 100×24", render(&mut a, 100, 24));
    assert!(quit.contains("Stop farming and quit?"));
    a.overlay = None;

    let small = show("too small 50×12", render(&mut a, 50, 12));
    assert!(small.contains("Make the window a little bigger"));
}

fn login(url: Option<&str>, scanned: bool) -> Overlay {
    Overlay::SignIn(SignInView {
        challenge: url.map(|u| LoginChallenge {
            url: u.into(),
            scanned,
        }),
        outcome: None,
        finished: None,
        from_account: false,
    })
}

/// Lets a background read land.
async fn settle(a: &mut App) {
    for _ in 0..200 {
        a.update();
        if !a.library.is_loading() {
            return;
        }
        tokio::task::yield_now().await;
    }
    panic!("still reading after 200 turns");
}

#[tokio::test]
async fn onboarding_previews() {
    let mut o = app(None, Preferences::default());
    assert_eq!(o.onboarding.step(), Some(Step::Welcome));
    let welcome = show("onboarding: welcome 100×28", render(&mut o, 100, 28));
    assert!(welcome.contains("Welcome to steamcards") && welcome.contains("agree and begin"));
    let small = show("onboarding: welcome 60×16", render(&mut o, 60, 16));
    assert!(
        small.contains("Farming cards is at your own risk.")
            && small.contains("to agree and begin"),
        "the notice whole, and how to go on"
    );

    o.onboarding_next();
    let sign_in = show("onboarding: sign in 100×28", render(&mut o, 100, 28));
    assert!(sign_in.contains("Sign in to Steam") && sign_in.contains("password is typed in here"));
    assert!(sign_in.contains("sign in first"), "can't go on without it");

    // Signed in: the games step reads the library.
    let mut g = app(cardfarmer(), prefs());
    g.onboarding.signed_out();
    g.onboarding.forward().unwrap();
    g.enter_step();
    assert_eq!(g.onboarding.step(), Some(Step::Games));
    let loading = show("onboarding: games, reading 100×28", render(&mut g, 100, 28));
    assert!(loading.contains("Reading your badges"));
    settle(&mut g).await;
    let games = show("onboarding: games 100×28", render(&mut g, 100, 28));
    assert!(games.contains("Pick your games") && games.contains("#1  Portal 2"));
    assert!(games.contains("3 cards to drop"));
    assert!(!games.contains("Half-Life 2"), "done games aren't offered");
    show("onboarding: games 60×16", render(&mut g, 60, 16));

    g.onboarding_next();
    let start = show("onboarding: ready 100×28", render(&mut g, 100, 28));
    assert!(start.contains("Ready to farm") && start.contains("#1 Portal 2"));
    assert!(start.contains("see you offline"));
    show("onboarding: ready 60×16", render(&mut g, 60, 16));

    g.overlay = Some(Overlay::Help);
    let help = show("help, onboarding 100×28", render(&mut g, 100, 28));
    assert!(help.contains("Setting up"));
}

/// The screen's rows as text.
fn rows(buf: &Buffer) -> Vec<String> {
    let area = buf.area;
    (0..area.height)
        .map(|y| (0..area.width).map(|x| buf[(x, y)].symbol()).collect())
        .collect()
}

/// Nothing is cut off at any size: every line of the dashboard is checked
/// against its area as it's drawn (a line too wide fails the test), and the
/// header keeps "appears offline" and the chosen game its bar.
#[tokio::test]
async fn nothing_is_cut_off_at_any_size() {
    let long = "Warhammer 40,000: Dawn of War II - Anniversary Edition";
    let mut states: Vec<(&str, App)> = Vec::new();
    states.push(("farming", farming_app()));
    let mut named = farming_app();
    if let Some(s) = named.status.as_mut() {
        let mut games: Vec<Game> = s.library.games().to_vec();
        games[0].name = long.into();
        s.library = SteamLibrary::new(games);
    }
    states.push(("a long name", named));
    let mut hours = farming_app();
    hours.status = Some(FarmingStatus {
        playing: vec![AppId(413_150), AppId(1_086_940), AppId(292_030)],
        mode: Some(Mode::Hours),
        ..farming_portal()
    });
    states.push(("building hours", hours));
    let mut blocked = farming_app();
    blocked.status = Some(FarmingStatus {
        status: Status::Blocked,
        playing: Vec::new(),
        mode: None,
        blocked_by: Some(AppId(730)),
        next_look: None,
        ..farming_portal()
    });
    states.push(("waiting", blocked));
    let mut grace = farming_app();
    grace.status = Some(FarmingStatus {
        status: Status::Blocked,
        playing: Vec::new(),
        mode: None,
        next_look: Some(now() + chrono::Duration::minutes(5)),
        ..farming_portal()
    });
    states.push(("waiting to carry on", grace));
    let mut idle = farming_app();
    idle.status = Some(FarmingStatus {
        status: Status::Idle,
        order: Vec::new(),
        playing: Vec::new(),
        mode: None,
        ..farming_portal()
    });
    states.push(("nothing to farm", idle));
    let mut paused = farming_app();
    paused.farming.pause();
    paused.paused_by_user = true;
    states.push(("paused", paused));
    let mut reading = app(cardfarmer(), prefs());
    reading.farming.start();
    states.push(("reading", reading));

    for (name, a) in &mut states {
        a.selected = Some(AppId(620));
        let widths = (60..=240).step_by(7).chain([99, 100, 209]);
        for w in widths {
            for h in [16, 19, 24, 30, 40, 49, 70] {
                let buf = render(a, w, h);
                let rows = rows(&buf);
                assert!(
                    rows[0].contains("appears offline"),
                    "{name} at {w}×{h}: the header lost \"appears offline\": {}",
                    rows[0]
                );
                if *name == "a long name" {
                    // Shown whole, or shortened at the end of a word.
                    for row in rows.iter().filter(|r| r.contains("Warhammer")) {
                        let from = row.find("Warhammer").unwrap_or(0);
                        let shown = &row[from..];
                        if shown.starts_with(long) {
                            continue;
                        }
                        let cut = shown
                            .find('…')
                            .unwrap_or_else(|| panic!("{name} at {w}×{h}: a name cut off: {row}"));
                        let kept = shown[..cut].trim_end();
                        let next = long[kept.len()..].chars().next();
                        assert!(
                            long.starts_with(kept) && next.is_none_or(|c| !c.is_alphanumeric()),
                            "{name} at {w}×{h}: a name cut mid-word: {row}"
                        );
                    }
                }
            }
        }
    }

    // The details, at the edges of every size.
    let mut a = farming_app();
    a.selected = Some(AppId(620));
    a.overlay = Some(Overlay::Detail { scroll: 0 });
    for (w, h) in [
        (60, 16),
        (60, 70),
        (240, 16),
        (240, 70),
        (99, 25),
        (100, 26),
    ] {
        let rows = rows(&render(&mut a, w, h));
        let text = rows.join("\n");
        assert!(
            text.contains("Farm priority") || text.contains("more ↓"),
            "details at {w}×{h}: what doesn't fit must say so"
        );
    }
}

// ── The README's screenshots ─────────────────────────────────────────────────
//
// A fuller library than the previews above, so the images look like a real
// afternoon's farming. Made-up account; real games, with made-up prices.

/// (app ID, name, hours, drops received, drops to come, a set's price in
/// pence).
const SHOWCASE: [(u32, &str, f64, u32, u32, i64); 27] = [
    (960_910, "Heavy Rain", 4.0, 3, 1, 5),
    (48_000, "LIMBO", 3.4, 3, 2, 6),
    (457_140, "Oxygen Not Included", 3.4, 3, 2, 7),
    (916_440, "Anno 1800", 3.5, 5, 3, 9),
    (247_080, "Crypt of the NecroDancer", 3.4, 4, 3, 5),
    (220_200, "Kerbal Space Program", 3.4, 4, 3, 6),
    (414_700, "Outlast 2", 3.4, 4, 3, 5),
    (1_118_200, "People Playground", 3.4, 4, 3, 4),
    (
        1_889_620,
        "We Were Here Expeditions: The FriendShip",
        8.7,
        3,
        3,
        4,
    ),
    (610_370, "Desperados III", 3.4, 5, 4, 7),
    (599_140, "Graveyard Keeper", 3.4, 5, 4, 5),
    (1_446_780, "MONSTER HUNTER RISE", 3.4, 5, 4, 8),
    (2_218_750, "Halls of Torment", 3.4, 6, 5, 4),
    (1_313_140, "Cult of the Lamb", 3.4, 7, 6, 6),
    (1_259_420, "Days Gone", 3.4, 7, 6, 9),
    (860_510, "Little Nightmares II", 3.4, 8, 7, 6),
    (1_102_190, "Monster Train", 3.4, 8, 7, 5),
    (3_070_070, "TCG Card Shop Simulator", 2.2, 4, 2, 4),
    (304_390, "FOR HONOR", 2.1, 6, 5, 5),
    (1_336_490, "Against the Storm", 1.6, 6, 3, 6),
    (1_332_010, "Stray", 1.4, 3, 1, 14),
    (391_540, "Undertale", 0.7, 3, 3, 12),
    (367_520, "Hollow Knight", 6.1, 4, 0, 9),
    (1_092_790, "Inscryption", 6.5, 4, 0, 5),
    (1_145_360, "Hades", 8.3, 4, 0, 8),
    (504_230, "Celeste", 6.2, 4, 0, 6),
    (557_600, "Gorogoa", 5.1, 3, 0, 4),
];

fn showcase_library() -> SteamLibrary {
    SteamLibrary::new(
        SHOWCASE
            .iter()
            .map(|&(app_id, name, hours, received, remaining, _)| {
                game(app_id, name, hours, received, remaining)
            })
            .collect(),
    )
}

/// The sets looked at: Heavy Rain's, being farmed.
fn showcase_sets() -> CardSets {
    let mut sets = CardSets::default();
    sets.update(
        AppId(960_910),
        CardSet::new(
            CardKind::Normal,
            vec![
                card("Ethan", 0),
                card("Carter", 0),
                card("Madison", 2),
                card("Norman", 0),
                card("Scott", 1),
            ],
        ),
    );
    sets
}

fn showcase_prices() -> PriceBook {
    let mut book = PriceBook::default();
    for &(app_id, _, _, _, _, pence) in &SHOWCASE {
        let set = match app_id {
            960_910 => set_prices(
                app_id,
                &[
                    ("Ethan", 5),
                    ("Carter", 4),
                    ("Madison", 5),
                    ("Norman", 6),
                    ("Scott", 4),
                ],
                &[("Madison", 60)],
                at(13, 0),
            ),
            1_145_360 => set_prices(
                app_id,
                &[("Zagreus", 8), ("Megaera", 7), ("Thanatos", 9), ("Nyx", 9)],
                &[("Thanatos", 62)],
                at(13, 0),
            ),
            367_520 => set_prices(
                app_id,
                &[("Hornet", 9), ("Zote", 7), ("The Knight", 11)],
                &[],
                at(13, 0),
            ),
            1_092_790 => set_prices(
                app_id,
                &[("Leshy", 6), ("Stoat", 5), ("Stinkbug", 5)],
                &[],
                at(13, 0),
            ),
            504_230 => set_prices(app_id, &[("Badeline", 6)], &[], at(13, 0)),
            557_600 => set_prices(app_id, &[("The Boy", 4)], &[], at(13, 0)),
            _ => set_prices(
                app_id,
                &[("A", pence), ("B", pence + 1), ("C", pence - 1)],
                &[],
                at(13, 0),
            ),
        };
        book.sets.insert(AppId(app_id), set);
    }
    book
}

fn showcase_session() -> Session {
    let named = |name: &str| DropCard::NameOnly { name: name.into() };
    let drop = |(h, m): (u32, u32), app_id: u32, card: DropCard, copy: Option<u32>| Drop {
        at: at(h, m),
        app_id: AppId(app_id),
        card,
        copy,
    };
    let stretch = |app_id: u32, from: (u32, u32), to: Option<(u32, u32)>| Stretch {
        app_ids: vec![AppId(app_id)],
        mode: Mode::Cards,
        from: at(from.0, from.1),
        to: to.map(|(h, m)| at(h, m)),
    };
    Session {
        started_at: at(7, 10),
        drops: vec![
            drop((7, 40), 367_520, named("Hornet"), Some(1)),
            drop((8, 8), 367_520, named("Zote"), Some(1)),
            drop((8, 37), 367_520, named("The Knight"), Some(1)),
            drop((9, 11), 1_092_790, named("Leshy"), Some(1)),
            drop((9, 39), 1_092_790, named("Stoat"), Some(1)),
            drop((10, 16), 1_092_790, named("Stinkbug"), Some(1)),
            drop(
                (10, 54),
                1_145_360,
                hades_card(21, "Zagreus", CardKind::Normal),
                Some(1),
            ),
            drop(
                (11, 26),
                1_145_360,
                hades_card(22, "Zagreus", CardKind::Normal),
                Some(2),
            ),
            drop(
                (11, 58),
                1_145_360,
                hades_card(23, "Thanatos", CardKind::Foil),
                Some(1),
            ),
            drop(
                (12, 31),
                1_145_360,
                hades_card(24, "Nyx", CardKind::Normal),
                Some(1),
            ),
            drop((12, 49), 504_230, named("Madeline"), Some(1)),
            drop((13, 12), 504_230, named("Badeline"), Some(1)),
            drop((13, 24), 557_600, named("The Boy"), Some(1)),
            drop((13, 38), 557_600, named("The Fruit"), Some(1)),
            drop((13, 57), 960_910, named("Madison"), Some(2)),
            drop((14, 3), 960_910, DropCard::Identifying, None),
        ],
        stretches: vec![
            stretch(367_520, (7, 10), Some((8, 37))),
            stretch(1_092_790, (8, 38), Some((10, 16))),
            stretch(1_145_360, (10, 17), Some((12, 31))),
            stretch(504_230, (12, 32), Some((13, 12))),
            stretch(557_600, (13, 13), Some((13, 38))),
            stretch(960_910, (13, 39), None),
        ],
        ..Default::default()
    }
}

/// Farming Heavy Rain, ranked first, with LIMBO next.
fn showcase_app() -> App {
    let prefs = Preferences {
        priority_games: vec![AppId(960_910), AppId(48_000)],
        ..Default::default()
    };
    let repo = Arc::new(FakePreferencesRepository::new(prefs));
    let get = Arc::new(DefaultGetPreferencesUseCase::new(repo.clone()));
    let accounts = Arc::new(StubGetAccountUseCase::new(cardfarmer()));
    let mut a = App::new(
        AccountViewModel::new(
            accounts.clone(),
            Arc::new(SpyCheckSignInUseCase::default()),
            Arc::new(SpySignOutUseCase::default()),
        ),
        SignInViewModel::new(Arc::new(StubSignInUseCase)),
        FarmingViewModel::new(
            Arc::new(SpyFarmCardsUseCase::default()),
            Arc::new(SpyEndSessionUseCase::default()),
            accounts.clone(),
            get.clone(),
            Arc::new(DefaultSetGameTierUseCase::new(repo.clone())),
        ),
        GamesViewModel::new(
            get,
            Arc::new(DefaultSetGameTierUseCase::new(repo.clone())),
            Arc::new(DefaultSetOnlyPriorityUseCase::new(repo.clone())),
            Arc::new(DefaultSetAppearOnlineUseCase::new(repo.clone())),
            Arc::new(DefaultSetRestartGamesUseCase::new(repo.clone())),
        ),
        LibraryViewModel::new(Arc::new(SpyGetLibraryUseCase::answering(Ok(
            showcase_library(),
        )))),
        OnboardingViewModel::new(accounts),
        MarketViewModel::new(
            Arc::new(StubGetCardPricesUseCase::new(showcase_prices())),
            Arc::new(SpySetCardsToPriceUseCase::default()),
            Arc::new(SpyKeepCardPricesUpToDateUseCase::default()),
            Arc::new(SpyRefreshCardPricesUseCase::default()),
            Arc::new(StubGetWalletUseCase::new(Some(pounds()))),
        ),
        UpdateViewModel::new(
            Arc::new(SpyKeepUpToDateUseCase::default()),
            Arc::new(DefaultGetPreferencesUseCase::new(repo.clone())),
            Arc::new(DefaultSetAutoUpdateUseCase::new(repo)),
        ),
    );
    a.clock = CLOCK;
    a.farming.start();
    let library = showcase_library();
    let order: Vec<AppId> = library
        .games()
        .iter()
        .filter(|g| g.has_drops_left())
        .map(|g| g.app_id)
        .collect();
    a.status = Some(FarmingStatus {
        status: Status::Farming,
        library,
        sets: showcase_sets(),
        order,
        playing: vec![AppId(960_910)],
        mode: Some(Mode::Cards),
        next_look: Some(now() + chrono::Duration::minutes(4)),
        session: showcase_session(),
        ..Default::default()
    });
    a.push_log(
        EventKind::Identified,
        "Madison dropped for Heavy Rain (a 2nd copy)".into(),
    );
    a.push_log(
        EventKind::Dropped,
        "A card dropped for Heavy Rain — 1 to go".into(),
    );
    a.selected = Some(AppId(960_910));
    a
}

/// The README's images: `cargo xtask screenshots` draws them from these.
#[tokio::test]
async fn readme_previews() {
    let mut a = showcase_app();
    let dashboard = show("readme: dashboard 146×31", render(&mut a, 146, 31));
    assert!(dashboard.contains("farming Heavy Rain") && dashboard.contains("Zagreus, 2nd"));
    assert!(dashboard.contains("★ Thanatos"));

    a.overlay = Some(Overlay::Detail { scroll: 0 });
    let details = show("readme: a game's details 120×34", render(&mut a, 120, 34));
    assert!(details.contains("Madison") && details.contains("×2"));
}
