// Renders every screen and state into an in-memory terminal with fake data, so
// layouts can be checked (and eyeballed) without touching a real account:
//
//   cargo test -p terminal-ui previews -- --nocapture

use std::sync::Arc;

use account::{
    Account as SignedIn, LoginChallenge,
    test_support::{fixed_account, no_refresh, recording_unlink, refusing_link},
};
use chrono::{DateTime, FixedOffset, Offset, TimeZone, Utc};
use farming::{EventKind, FarmingStatus, Mode, Status, test_support::idle_farmer};
use library::{Card, CardDrops, Game, SteamLibrary, test_support::fixed_library};
use preferences::{
    Preferences, get_preferences, set_appear_online, set_game_tier, set_only_priority,
    test_support::InMemoryPreferencesRepository,
};
use ratatui::{Terminal, backend::TestBackend, buffer::Buffer};

use super::{App, Clock, LogView, LoginView, Overlay};
use crate::viewmodel::{Account, Farming, Games, Library, Login, Onboarding, Step};

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
        app_id,
        name: name.into(),
        hours,
        drops: CardDrops {
            received,
            remaining,
        },
        badge_level: 0,
        cards: Vec::new(),
    }
}

/// A library like a real one: some games dropping, some building hours, some
/// done.
fn library() -> SteamLibrary {
    let mut portal = game(620, "Portal 2", 5.2, 1, 3);
    portal.badge_level = 1;
    portal.cards = vec![
        card("Atlas", 2),
        card("P-Body", 1),
        card("Wheatley", 0),
        card("GLaDOS", 0),
        card("Chell", 1),
        card("Space Core", 0),
        card("Cave Johnson", 0),
        card("Turret", 0),
    ];
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

fn card(name: &str, owned: u32) -> Card {
    Card {
        name: name.into(),
        owned,
    }
}

fn prefs() -> Preferences {
    Preferences {
        priority_games: vec![620, 413_150],
        skipped_games: vec![730],
        ..Default::default()
    }
}

fn app(account: Option<SignedIn>, prefs: Preferences) -> App {
    let repo = Arc::new(InMemoryPreferencesRepository::new(prefs));
    let get = get_preferences(repo.clone());
    let accounts = fixed_account(account);
    let mut app = App::new(
        Account::new(accounts.clone(), no_refresh(), recording_unlink().0),
        Login::new(refusing_link()),
        Farming::new(
            idle_farmer(),
            Arc::new(|| {}),
            accounts.clone(),
            get.clone(),
            set_game_tier(repo.clone()),
        ),
        Games::new(
            get,
            set_game_tier(repo.clone()),
            set_only_priority(repo.clone()),
            set_appear_online(repo),
        ),
        Library::new(fixed_library(library())),
        Onboarding::new(accounts),
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
        order: vec![620, 413_150, 1_145_360, 1_086_940, 292_030],
        playing: vec![620],
        mode: Some(Mode::Cards),
        blocked_by: None,
        next_look: Some(now() + chrono::Duration::minutes(12)),
        note: String::new(),
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

/// Prints the screen as text; returns the text for assertions.
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
    text
}

#[tokio::test]
async fn previews() {
    let mut a = farming_app();
    a.selected = Some(620);
    let wide = show("dashboard, farming 146×40", render(&mut a, 146, 40));
    assert!(wide.contains("Farming Portal 2") && wide.contains("#1"));
    assert!(wide.contains("next look in 12m"), "the Now panel says when");
    assert!(wide.contains("1/4 cards · 5.2h played"));
    assert!(wide.contains("PRIORITY") && wide.contains("farmed first, in your order"));
    assert!(wide.contains("SKIPPED") && wide.contains("never farmed"));
    assert!(wide.contains("DONE · 2"));
    assert!(wide.contains("▶ farming"), "the queue spells out status");
    assert!(wide.contains("Farming now"), "the details say it");
    assert!(
        wide.contains("3 of 8 collected"),
        "the card set, when known"
    );
    assert!(wide.contains("✓ Atlas ×2"));
    assert!(wide.contains("Farm priority"));
    assert!(
        wide.contains("  ┤"),
        "the selected row is joined to the details"
    );
    assert!(
        wide.contains("appears offline"),
        "the header says how friends see you"
    );
    assert!(wide.contains("A card dropped for Portal 2"));

    a.selected = Some(413_150);
    let queued = show(
        "dashboard, a game short of 3 hours 146×40",
        render(&mut a, 146, 40),
    );
    assert!(queued.contains("Next up") || queued.contains("Queued"));
    assert!(queued.contains("cards drop from 3h"));

    let narrow = show("dashboard 80×24", render(&mut a, 80, 24));
    assert!(
        narrow.contains("enter  details"),
        "narrow windows point to the pop-up"
    );

    a.show_done = true;
    show("done games shown 120×30", render(&mut a, 120, 30));
    a.show_done = false;

    a.flash("Portal 2 is now priority #1.");
    show("flash message 120×30", render(&mut a, 120, 30));

    a.overlay = Some(Overlay::Detail);
    let detail = show("details pop-up 80×24", render(&mut a, 80, 24));
    assert!(
        detail.contains("Portal 2 is now priority #1."),
        "a pop-up doesn't hide what a key just did"
    );
    a.flash = None;

    // Building hours: several games together.
    let mut b = farming_app();
    b.status = Some(FarmingStatus {
        playing: vec![413_150, 1_086_940, 292_030],
        mode: Some(Mode::Hours),
        next_look: Some(now() + chrono::Duration::minutes(54)),
        ..farming_portal()
    });
    b.selected = Some(1_086_940);
    let hours = show("dashboard, building hours 120×30", render(&mut b, 120, 30));
    assert!(hours.contains("Building hours on 3 games"));
    assert!(hours.contains("Stardew Valley ready in 54m"));
    assert!(hours.contains("▷ hours"));
    assert!(hours.contains("Building hours, with 2 others"));
    show("dashboard, building hours 80×24", render(&mut b, 80, 24));

    // Another device playing.
    let mut blocked = farming_app();
    blocked.status = Some(FarmingStatus {
        status: Status::Blocked,
        playing: Vec::new(),
        mode: None,
        blocked_by: Some(730),
        note: "playing on another device — farming waits until it stops".into(),
        ..farming_portal()
    });
    let text = show("playing elsewhere 120×30", render(&mut blocked, 120, 30));
    assert!(text.contains("Playing on another device: Counter-Strike 2"));

    // Nothing left.
    let mut idle = farming_app();
    idle.status = Some(FarmingStatus {
        status: Status::Idle,
        order: Vec::new(),
        playing: Vec::new(),
        mode: None,
        next_look: Some(now() + chrono::Duration::hours(8)),
        note: "every card has dropped".into(),
        ..farming_portal()
    });
    let text = show("nothing to farm 120×30", render(&mut idle, 120, 30));
    assert!(text.contains("Every card has dropped") && text.contains("looks again in 8h"));

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
    assert!(text.contains("‖ paused") && text.contains("to carry on"));

    // Steam rejected the sign-in.
    let mut expired = app(
        Some(SignedIn {
            name: "cardfarmer".into(),
            expired: true,
        }),
        prefs(),
    );
    let text = show("sign-in expired 120×30", render(&mut expired, 120, 30));
    assert!(text.contains("no longer takes the saved sign-in"));
    assert!(text.contains("sign in again"));

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
    a.overlay = Some(Overlay::Login(LoginView {
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
    show("help, short window 130×24", render(&mut a, 130, 24));

    a.overlay = Some(Overlay::ConfirmQuit);
    let quit = show("quit 100×24", render(&mut a, 100, 24));
    assert!(quit.contains("Stop farming and quit?"));
    a.overlay = None;

    let small = show("too small 50×12", render(&mut a, 50, 12));
    assert!(small.contains("Make the window a little bigger"));
}

fn login(url: Option<&str>, scanned: bool) -> Overlay {
    Overlay::Login(LoginView {
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
    show("onboarding: welcome 60×16", render(&mut o, 60, 16));

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
