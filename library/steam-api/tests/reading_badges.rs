//! Reading the badge pages: what each page says, and the whole list as a
//! session gathers it, signed in to a stand-in for steamcommunity.com.

use std::sync::Arc;

use config_file::{ConfigFile, CredentialStore, Credentials};
use debug_log::DebugLog;
use steam_api::{
    Session,
    badges::{BadgeGame, read_badge_page, read_game_cards_page},
    test_support::{ACCOUNT, FakeSteam, STEAM_ID, token},
};
use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{method, path, query_param},
};

fn page(name: &str) -> String {
    std::fs::read_to_string(format!("{}/tests/pages/{name}", env!("CARGO_MANIFEST_DIR"))).unwrap()
}

fn game(badges: &[BadgeGame], app_id: u32) -> BadgeGame {
    badges
        .iter()
        .find(|g| g.app_id == app_id)
        .cloned()
        .unwrap_or_else(|| panic!("{app_id} isn't on the page"))
}

#[test]
fn a_page_lists_each_games_cards_and_hours() {
    let page = read_badge_page(&page("badges-1.html"));

    assert_eq!(page.viewer, Some(STEAM_ID));
    assert_eq!(page.pages, 2);
    let ids: Vec<u32> = page.games.iter().map(|g| g.app_id).collect();
    assert_eq!(
        ids,
        [620, 440, 220, 413150],
        "the service badge isn't a game"
    );

    let portal = game(&page.games, 620);
    assert_eq!(portal.name, "Portal 2");
    assert_eq!(portal.hours, 5.2);
    assert_eq!(portal.cards_left, 3);
    assert_eq!(portal.cards_received, 1);
    assert_eq!(portal.badge_level, 1);
    assert!(!portal.unsure);

    let unplayed = game(&page.games, 413150);
    assert_eq!((unplayed.hours, unplayed.cards_left), (0.0, 4));
    assert_eq!(unplayed.badge_level, 0, "no badge crafted yet");
}

#[test]
fn a_finished_game_has_nothing_left() {
    let page = read_badge_page(&page("badges-1.html"));
    let done = game(&page.games, 220);
    assert_eq!(done.name, "Half-Life 2");
    assert_eq!((done.cards_left, done.cards_received), (0, 3));
    assert!(!done.unsure);
}

#[test]
fn a_free_to_play_game_the_badge_may_be_wrong_about_is_checked() {
    let page = read_badge_page(&page("badges-1.html"));
    let tf2 = game(&page.games, 440);
    assert_eq!(tf2.hours, 1234.5, "commas and all");
    assert_eq!(tf2.cards_left, 0);
    assert!(
        tf2.unsure,
        "no drops and none received: ASF asks its own page"
    );
}

#[test]
fn a_page_shown_signed_out_says_so() {
    let page = read_badge_page(&page("badges-signed-out.html"));
    assert_eq!(page.viewer, None);
}

#[test]
fn a_games_own_page_reads_the_same() {
    let cards = read_game_cards_page(730, &page("gamecards-730.html"));
    let cs = cards.game.unwrap();
    assert_eq!(cs.name, "Counter-Strike 2");
    assert_eq!((cs.hours, cs.cards_left), (350.1, 2));
    assert_eq!(cards.viewer, Some(STEAM_ID));
    assert!(read_game_cards_page(1, "<html></html>").game.is_none());
}

/// A session signed in as the stand-in's account, reading pages from
/// `site`.
async fn session(steam: &FakeSteam, site: &MockServer, name: &str) -> Session {
    let dir = std::env::temp_dir().join(format!("steamcards-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let store = Arc::new(ConfigFile::open(dir.join("config.json")).unwrap());
    store
        .save_credentials(Credentials {
            refresh_token: token(STEAM_ID, 4_000_000_000),
            account_name: ACCOUNT.into(),
            steam_id: STEAM_ID,
            login_id: 7,
        })
        .unwrap();
    let mut endpoints = steam.endpoints();
    endpoints.community = site.uri();
    Session::with_endpoints(store, &DebugLog::off(), endpoints)
}

async fn serve(site: &MockServer, at: &str, page: String, query: Option<(&str, &str)>) {
    let mock = Mock::given(method("GET")).and(path(at.to_owned()));
    let mock = match query {
        Some((k, v)) => mock.and(query_param(k, v)),
        None => mock,
    };
    mock.respond_with(ResponseTemplate::new(200).set_body_string(page))
        .mount(site)
        .await;
}

#[tokio::test]
async fn every_page_is_read_and_unsure_games_checked() {
    let steam = FakeSteam::start().await;
    let site = MockServer::start().await;
    let badges = format!("/profiles/{STEAM_ID}/badges");
    serve(&site, &badges, page("badges-1.html"), Some(("p", "1"))).await;
    serve(&site, &badges, page("badges-2.html"), Some(("p", "2"))).await;
    serve(
        &site,
        &format!("/profiles/{STEAM_ID}/gamecards/730"),
        page("gamecards-730.html"),
        None,
    )
    .await;
    serve(
        &site,
        &format!("/profiles/{STEAM_ID}/gamecards/440"),
        "<html></html>".into(),
        None,
    )
    .await;
    let session = session(&steam, &site, "badges").await;

    let games = session.badges().await.unwrap();

    let ids: Vec<u32> = games.iter().map(|g| g.app_id).collect();
    assert_eq!(ids, [620, 440, 220, 413150, 730, 1145360]);
    let cs = game(&games, 730);
    assert_eq!(cs.cards_left, 2, "its own page knew better");
    assert!(!cs.unsure);
    assert!(!game(&games, 440).unsure, "checked, and nothing to farm");
    assert_eq!(
        game(&games, 1145360).cards_left,
        1,
        "\"1 card drop remaining\""
    );
}

#[tokio::test]
async fn a_page_shown_signed_out_gets_a_new_token_then_gives_up() {
    let steam = FakeSteam::start().await;
    let site = MockServer::start().await;
    serve(
        &site,
        &format!("/profiles/{STEAM_ID}/badges"),
        page("badges-signed-out.html"),
        None,
    )
    .await;
    let session = session(&steam, &site, "signedout").await;

    let e = session.badges().await.unwrap_err();

    assert!(e.to_string().contains("wouldn't take the sign-in"), "{e}");
    let asked = site.received_requests().await.unwrap().len();
    assert_eq!(asked, 2, "once with the token at hand, once with a new one");
}

#[tokio::test]
async fn a_renewed_sign_in_is_saved() {
    let steam = FakeSteam::start().await;
    steam.renew_refresh_tokens();
    let site = MockServer::start().await;
    serve(
        &site,
        &format!("/profiles/{STEAM_ID}/gamecards/730"),
        page("gamecards-730.html"),
        None,
    )
    .await;
    let session = session(&steam, &site, "renew").await;
    let before = session.credentials().unwrap().refresh_token;

    session.game_cards(730).await.unwrap();

    let after = session.credentials().unwrap().refresh_token;
    assert_ne!(after, before, "Steam's new refresh token replaces the old");
}
