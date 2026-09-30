// The pop-ups held to the spec's mockups (docs/design/ui.md §3, mockups h to
// m and the log), character for character, colour aside; and their keys.

use crossterm::event::{KeyCode, KeyEvent};

use super::{
    super::{App, fixtures, golden},
    Overlay,
};

/// Holds the screen the mockup titled `title` shows to it.
fn golden(title: &str) {
    let mut app = fixtures::for_mockup(title).expect("a fixture");
    golden::check(title, &mut app);
}

fn press(app: &mut App, code: KeyCode) {
    app.key(KeyEvent::from(code));
}

#[tokio::test]
async fn the_game_details_are_the_spec_s() {
    golden("game details");
}

#[tokio::test]
async fn the_game_details_scroll_at_60_by_16() {
    golden("game details (XS)");
}

#[tokio::test]
async fn this_sessions_cards_are_the_spec_s() {
    golden("this session's haul");
}

#[tokio::test]
async fn the_market_view_is_the_spec_s() {
    golden("the market view, prices paused");
}

#[tokio::test]
async fn help_is_the_spec_s() {
    golden("help");
}

#[tokio::test]
async fn games_and_settings_are_the_spec_s() {
    golden("games and settings");
}

#[tokio::test]
async fn signing_in_is_the_spec_s() {
    golden("sign in with a QR code, scanned");
}

#[tokio::test]
async fn the_account_is_the_spec_s() {
    golden("account");
}

#[tokio::test]
async fn the_log_is_the_spec_s_at_each_size() {
    for title in ["the log", "the log (S)", "the log (XS)"] {
        golden(title);
    }
}

/// Signing in: with a code to scan or not yet, and how it went.
fn login(challenge: bool, outcome: Option<Result<String, String>>) -> Overlay {
    Overlay::Login(super::LoginView {
        challenge: challenge.then(|| ::account::LoginChallenge {
            url: "https://s.team/q/1/12539683334892375075".into(),
            scanned: false,
        }),
        outcome,
        finished: None,
        from_account: false,
    })
}

/// Opens a pop-up.
type Opens = fn() -> Overlay;

/// Every pop-up, and each of its states that looks different.
fn every_pop_up() -> Vec<(&'static str, Opens)> {
    vec![
        ("details", || Overlay::Details(super::Scroll::default())),
        ("haul", || Overlay::Haul(super::HaulView::default())),
        ("market", || Overlay::Market(super::MarketList::default())),
        ("games", || Overlay::Games(super::GamesView::default())),
        ("account", || Overlay::Account { confirm: false }),
        ("sign out", || Overlay::Account { confirm: true }),
        ("sign in", || login(true, None)),
        ("asking", || login(false, None)),
        ("signed in", || login(false, Some(Ok("alice".into())))),
        ("failed", || {
            login(
                false,
                Some(Err(
                    "Steam didn't answer in time: the connection was reset".into()
                )),
            )
        }),
        ("log", || Overlay::Log(super::LogView::default())),
        ("help", || Overlay::Help(super::Scroll::default())),
        ("quit", || Overlay::ConfirmQuit),
    ]
}

#[tokio::test]
async fn every_pop_up_fits_at_the_edges_of_each_size_class() {
    // Each pop-up over the farming state (and the market over prices paused
    // by Steam, with its banner) at the edges of every size class: a line
    // wider than its room, or a ladder with no rung that fits, fails the
    // render.
    let sweep = |name: &str, open: Opens, mut app: App| {
        for w in [60, 71, 72, 99, 100, 120, 199, 200, 240] {
            for h in [16, 19, 20, 25, 26, 35, 36, 39, 40, 70] {
                app.overlay = Some(open());
                let screen = golden::screen_text(&golden::render(&mut app, w, h));
                assert_eq!(screen.len(), usize::from(h), "{name} at {w}×{h}");
            }
        }
    };
    for (name, open) in every_pop_up() {
        sweep(name, open, fixtures::farming_alone());
    }
    sweep(
        "market, paused",
        || Overlay::Market(super::MarketList::default()),
        fixtures::prices_paused(),
    );
}

#[tokio::test]
async fn every_pop_up_fits_over_every_state() {
    // No cards yet and prices still coming; a five-day haul of 252 cards,
    // nothing left to farm; nothing known while the badges are read; an
    // expired sign-in; a group building hours.
    let states = [
        fixtures::first_minutes as fn() -> App,
        fixtures::nothing_to_farm,
        fixtures::reading_badges,
        fixtures::sign_in_expired,
        fixtures::building_hours,
    ];
    for state in states {
        let mut app = state();
        for (name, open) in every_pop_up() {
            for (w, h) in [(60, 16), (80, 24), (120, 30), (200, 50)] {
                app.overlay = Some(open());
                let screen = golden::screen_text(&golden::render(&mut app, w, h));
                assert_eq!(screen.len(), usize::from(h), "{name} at {w}×{h}");
            }
        }
    }
}

#[tokio::test]
async fn whats_behind_a_pop_up_is_faded_but_the_header_and_strip_arent() {
    use ratatui::style::Modifier;
    let mut app = fixtures::for_mockup("account").unwrap();
    let buf = golden::render(&mut app, 120, 30);
    let faded = |x: u16, y: u16| buf[(x, y)].modifier.contains(Modifier::DIM);
    assert!(!faded(2, 0), "the header");
    assert!(faded(10, 10), "the queue");
    assert!(!faded(80, 10), "the pop-up, in the right-hand column");
    assert!(!faded(2, 28), "the strip");
    assert!(faded(2, 29), "the footer");
}

#[tokio::test]
async fn a_pop_up_over_the_sessions_summary_leaves_progress_whole() {
    // With nothing left to farm, Progress takes seven rows at M (mockup e):
    // the account goes over the right-hand column below it, not over its
    // last rows.
    let mut app = fixtures::nothing_to_farm();
    app.overlay = Some(Overlay::Account { confirm: false });
    let screen = golden::screen_text(&golden::render(&mut app, 120, 30));
    assert_eq!(
        screen[9],
        format!("╰{}╯", "─".repeat(118)),
        "Progress's foot"
    );
    let right: String = screen[10].chars().skip(74).collect();
    assert!(right.starts_with("╭ Account ─"), "{}", screen[10]);
}

#[tokio::test]
async fn the_details_scroll_a_page_at_a_time() {
    let title = "game details (XS)";
    let mut app = fixtures::for_mockup(title).unwrap();
    let bottom = |app: &mut App| golden::screen_text(&golden::render(app, 60, 16))[13].clone();
    assert!(bottom(&mut app).contains("15 more ↓ [PgDn]"));
    press(&mut app, KeyCode::PageDown);
    assert!(
        bottom(&mut app).contains("4 more ↓ [PgDn]"),
        "{}",
        bottom(&mut app)
    );
    press(&mut app, KeyCode::End);
    assert!(!bottom(&mut app).contains("more ↓"), "{}", bottom(&mut app));
    press(&mut app, KeyCode::Home);
    assert!(bottom(&mut app).contains("15 more ↓"));
}

#[tokio::test]
async fn the_log_scrolls_back_and_follows_the_newest_again() {
    let mut app = fixtures::for_mockup("the log").unwrap();
    let screen = |app: &mut App| golden::screen_text(&golden::render(app, 120, 30));
    assert!(screen(&mut app)[1].contains("211 events · following the newest"));
    press(&mut app, KeyCode::Up);
    let back = screen(&mut app);
    assert!(back[1].ends_with("211 events ─╮"), "{}", back[1]);
    assert!(
        back[27].contains("185 earlier ↑ · 1 later ↓"),
        "{}",
        back[27]
    );
    press(&mut app, KeyCode::End);
    assert!(screen(&mut app)[27].contains("186 earlier ↑"));
    press(&mut app, KeyCode::Home);
    let top = screen(&mut app);
    assert!(top[2].contains("09:14:00"), "the oldest first: {}", top[2]);
}

#[tokio::test]
async fn a_stale_set_says_its_age_once() {
    let mut app = fixtures::prices_paused();
    app.overlay = app.open_details();
    let screen = golden::screen_text(&golden::render(&mut app, 120, 30));
    let find = |text: &str| {
        screen
            .iter()
            .find(|row| row.contains(text))
            .cloned()
            .unwrap_or_default()
    };
    assert!(
        find("── The set").contains("a badge takes one of each · 8h old ─"),
        "{}",
        find("── The set")
    );
    assert!(
        find("×2").contains("Madison         ×2   £0.05   £0.60     £0.02"),
        "{}",
        find("×2")
    );
    assert!(find("Prices from the Steam market").contains("8h ago: over 6 hours old"));
}

#[tokio::test]
async fn the_haul_and_the_market_choose_with_the_arrows() {
    let mut app = fixtures::farming_alone();
    app.overlay = Some(Overlay::Haul(super::HaulView::default()));
    press(&mut app, KeyCode::Up);
    let Some(Overlay::Haul(v)) = &app.overlay else {
        panic!("the haul closed");
    };
    assert_eq!(v.card, Some(14), "from the newest card, the one before");

    app.overlay = Some(Overlay::Market(super::MarketList::default()));
    press(&mut app, KeyCode::Down);
    let Some(Overlay::Market(v)) = &app.overlay else {
        panic!("the market closed");
    };
    assert_eq!(
        v.game,
        Some(crate::viewmodel::fixtures::LIMBO),
        "from the chosen game, the next"
    );
    press(&mut app, KeyCode::Char('r'));
    assert!(
        app.flash
            .as_ref()
            .is_some_and(|(f, _)| f.starts_with("LIMBO's prices are looked up again")),
        "{:?}",
        app.flash
    );
}

#[tokio::test]
async fn each_pop_up_closes_with_its_own_keys() {
    let cases: [(fn() -> Overlay, KeyCode); 6] = [
        (
            || Overlay::Help(super::Scroll::default()),
            KeyCode::Char('?'),
        ),
        (
            || Overlay::Haul(super::HaulView::default()),
            KeyCode::Char('h'),
        ),
        (
            || Overlay::Market(super::MarketList::default()),
            KeyCode::Char('m'),
        ),
        (
            || Overlay::Log(super::LogView::default()),
            KeyCode::Char('l'),
        ),
        (
            || Overlay::Games(super::GamesView::default()),
            KeyCode::Char('g'),
        ),
        (
            || Overlay::Details(super::Scroll::default()),
            KeyCode::Enter,
        ),
    ];
    for (open, close) in cases {
        let mut app = fixtures::farming_alone();
        app.overlay = Some(open());
        press(&mut app, close);
        assert!(app.overlay.is_none());
    }
}

#[tokio::test]
async fn the_details_move_to_the_next_game_without_closing() {
    let mut app = fixtures::farming_alone();
    app.overlay = app.open_details();
    press(&mut app, KeyCode::Down);
    assert!(matches!(app.overlay, Some(Overlay::Details(_))));
    assert_eq!(app.selected, Some(crate::viewmodel::fixtures::LIMBO));
}

#[tokio::test]
async fn quick_sells_keys_do_nothing_yet() {
    // s, k and u in this session's cards; tab, space and ←→ in the market.
    for code in [KeyCode::Char('s'), KeyCode::Char('k'), KeyCode::Char('u')] {
        let mut app = fixtures::farming_alone();
        app.overlay = Some(Overlay::Haul(super::HaulView::default()));
        press(&mut app, code);
        assert!(
            matches!(app.overlay, Some(Overlay::Haul(v)) if v == super::HaulView::default()),
            "{code:?}"
        );
        assert!(app.flash.is_none(), "{code:?}");
    }
    for code in [
        KeyCode::Tab,
        KeyCode::Char(' '),
        KeyCode::Left,
        KeyCode::Right,
    ] {
        let mut app = fixtures::farming_alone();
        app.overlay = Some(Overlay::Market(super::MarketList::default()));
        press(&mut app, code);
        assert!(
            matches!(app.overlay, Some(Overlay::Market(v)) if v == super::MarketList::default()),
            "{code:?}"
        );
        assert!(app.flash.is_none(), "{code:?}");
    }
}
