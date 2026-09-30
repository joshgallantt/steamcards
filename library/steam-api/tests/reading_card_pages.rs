//! Reading a game's card page: what it says, and how a session fetches it,
//! signed in to a stand-in for steamcommunity.com.

use std::sync::Arc;

use config_file::{ConfigFile, CredentialStore, Credentials};
use debug_log::DebugLog;
use steam_api::{
    SteamClient,
    badges::{read_foil_cards_page, read_game_cards_page},
    test_support::{ACCOUNT, FakeSteam, STEAM_ID, token},
};
use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{method, path, query_param},
};

fn page(name: &str) -> String {
    std::fs::read_to_string(format!("{}/tests/pages/{name}", env!("CARGO_MANIFEST_DIR"))).unwrap()
}

#[test]
fn a_games_own_page_reads_the_same() {
    let cards = read_game_cards_page(730, &page("gamecards-730.html"));
    let cs = cards.game.unwrap();
    assert_eq!(cs.name, "Counter-Strike 2");
    assert_eq!((cs.hours, cs.cards_left), (350.1, 2));
    assert_eq!(cards.viewer, Some(STEAM_ID));
    let set: Vec<(&str, u32)> = cs
        .cards
        .iter()
        .map(|c| (c.name.as_str(), c.owned))
        .collect();
    assert_eq!(
        set,
        [
            ("Anarchist", 2),
            ("Balkan", 1),
            ("FBI", 0),
            ("Phoenix", 0),
            ("SAS", 0)
        ],
        "the set, and how many of each"
    );
    assert!(read_game_cards_page(1, "<html></html>").game.is_none());
}

#[test]
fn a_games_foils_are_on_a_page_of_their_own() {
    let foils: Vec<(String, u32)> = read_foil_cards_page(&page("gamecards-730-foil.html"))
        .into_iter()
        .map(|c| (c.name, c.owned))
        .collect();
    let foils: Vec<(&str, u32)> = foils.iter().map(|(n, o)| (n.as_str(), *o)).collect();
    assert_eq!(
        foils,
        [
            ("Anarchist", 2),
            ("Balkan", 0),
            ("FBI", 1),
            ("Phoenix", 0),
            ("SAS", 0)
        ],
        "each foil, named as the set names the card, and how many of it"
    );
    assert!(read_foil_cards_page("<html></html>").is_empty());
}

/// A session signed in as the stand-in's account, reading pages from
/// `site`.
async fn session(steam: &FakeSteam, site: &MockServer, name: &str) -> SteamClient {
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
    SteamClient::with_endpoints(store, &DebugLog::off(), endpoints)
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
async fn a_games_foils_are_read_signed_in_from_its_foil_badge() {
    let steam = FakeSteam::start().await;
    let site = MockServer::start().await;
    serve(
        &site,
        &format!("/profiles/{STEAM_ID}/gamecards/730"),
        page("gamecards-730-foil.html"),
        Some(("border", "1")),
    )
    .await;
    let session = session(&steam, &site, "foils").await;

    let foils = session.foil_cards(730).await.unwrap();

    assert_eq!(foils.len(), 5);
    assert_eq!((foils[0].name.as_str(), foils[0].owned), ("Anarchist", 2));
}

#[tokio::test]
async fn a_page_shown_signed_out_gets_a_new_token_then_gives_up() {
    let steam = FakeSteam::start().await;
    let site = MockServer::start().await;
    serve(
        &site,
        &format!("/profiles/{STEAM_ID}/gamecards/730"),
        page("signed-out.html"),
        None,
    )
    .await;
    let session = session(&steam, &site, "signedout").await;

    let e = session.game_cards(730).await.unwrap_err();

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
