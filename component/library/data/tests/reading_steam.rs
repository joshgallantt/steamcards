//! The library as steamcommunity.com and the inventory show it, against
//! stand-ins for Steam and the site.

use std::sync::Arc;

use config_file::{ConfigFile, CredentialStore, Credentials};
use debug_log::DebugLog;
use library::{Card, CardAsset, CardDrops, Game, LibraryRepository};
use library_data::SteamLibraryRepository;
use steam_api::{
    Session,
    test_support::{ACCOUNT, FakeSteam, HeldItem, STEAM_ID, token},
};
use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{method, path, query_param},
};

fn fixture(name: &str) -> String {
    let at = format!(
        "{}/../../../library/steam-api/tests/pages/{name}",
        env!("CARGO_MANIFEST_DIR")
    );
    std::fs::read_to_string(at).unwrap()
}

async fn serving(site: &MockServer, at: String, page: String, query: Option<(&str, &str)>) {
    let mock = Mock::given(method("GET")).and(path(at));
    let mock = match query {
        Some((k, v)) => mock.and(query_param(k, v)),
        None => mock,
    };
    mock.respond_with(ResponseTemplate::new(200).set_body_string(page))
        .mount(site)
        .await;
}

async fn repository(steam: &FakeSteam, site: &MockServer, name: &str) -> SteamLibraryRepository {
    let dir =
        std::env::temp_dir().join(format!("steamcards-library-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let file = Arc::new(ConfigFile::open(dir.join("config.json")).unwrap());
    file.save_credentials(Credentials {
        refresh_token: token(STEAM_ID, 4_000_000_000),
        account_name: ACCOUNT.into(),
        steam_id: STEAM_ID,
        login_id: 7,
    })
    .unwrap();
    let mut endpoints = steam.endpoints();
    endpoints.community = site.uri();
    let session = Session::with_endpoints(file, &DebugLog::off(), endpoints);
    SteamLibraryRepository::new(Arc::new(session))
}

#[tokio::test]
async fn the_badges_are_the_library() {
    let steam = FakeSteam::start().await;
    let site = MockServer::start().await;
    let badges = format!("/profiles/{STEAM_ID}/badges");
    serving(
        &site,
        badges.clone(),
        fixture("badges-1.html"),
        Some(("p", "1")),
    )
    .await;
    serving(&site, badges, fixture("badges-2.html"), Some(("p", "2"))).await;
    serving(
        &site,
        format!("/profiles/{STEAM_ID}/gamecards/730"),
        fixture("gamecards-730.html"),
        None,
    )
    .await;
    serving(
        &site,
        format!("/profiles/{STEAM_ID}/gamecards/440"),
        "<html></html>".into(),
        None,
    )
    .await;
    let repo = repository(&steam, &site, "badges").await;

    let library = repo.library().await.unwrap();

    assert_eq!(
        library.game(620),
        Some(&Game {
            app_id: 620,
            name: "Portal 2".into(),
            hours: 5.2,
            drops: CardDrops {
                received: 1,
                remaining: 3,
            },
            badge_level: 1,
            cards: Vec::new(),
        })
    );
    assert_eq!(library.with_drops_left().count(), 5);
    assert_eq!(library.drops_left(), 3 + 6 + 4 + 2 + 1);
    let never_played = library.game(1086940).unwrap();
    assert_eq!(
        (never_played.drops.received, never_played.drops.total()),
        (0, 6),
        "nothing has dropped from a game never played"
    );
}

#[tokio::test]
async fn a_games_own_page_has_its_card_set() {
    let steam = FakeSteam::start().await;
    let site = MockServer::start().await;
    serving(
        &site,
        format!("/profiles/{STEAM_ID}/gamecards/730"),
        fixture("gamecards-730.html"),
        None,
    )
    .await;
    let repo = repository(&steam, &site, "cards").await;

    let cs = repo.game(730).await.unwrap();

    assert_eq!(cs.drops.remaining, 2);
    assert_eq!(cs.cards.len(), 5);
    assert_eq!(
        cs.cards[0],
        Card {
            name: "Anarchist".into(),
            owned: 2,
        }
    );
    assert_eq!(cs.cards_collected(), 2);
}

#[tokio::test]
async fn a_games_foils_are_counted_on_a_page_of_their_own() {
    let steam = FakeSteam::start().await;
    let site = MockServer::start().await;
    serving(
        &site,
        format!("/profiles/{STEAM_ID}/gamecards/730"),
        fixture("gamecards-730-foil.html"),
        Some(("border", "1")),
    )
    .await;
    let repo = repository(&steam, &site, "foils").await;

    let foils = repo.foils(730).await.unwrap();

    assert_eq!(
        foils[..3],
        [
            Card {
                name: "Anarchist".into(),
                owned: 2,
            },
            Card {
                name: "Balkan".into(),
                owned: 0,
            },
            Card {
                name: "FBI".into(),
                owned: 1,
            },
        ]
    );
}

#[tokio::test]
async fn new_items_are_described_as_the_cards_they_are() {
    let steam = FakeSteam::start().await;
    steam.hold(vec![
        HeldItem::card(31_001, 960_910, "Madison"),
        HeldItem::card(31_002, 960_910, "Madison"),
        HeldItem::foil(31_003, 960_910, "Scott (Foil)"),
        HeldItem::other(31_004, 960_910, 4, ":origami:"),
    ]);
    let site = MockServer::start().await;
    let repo = repository(&steam, &site, "describe").await;

    let cards = repo
        .describe(&[31_001, 31_002, 31_003, 31_004])
        .await
        .unwrap();

    let madison = |asset_id| CardAsset {
        asset_id,
        app_id: 960_910,
        name: "Madison".into(),
        market_hash_name: "960910-Madison".into(),
        foil: false,
        marketable: true,
        tradable: true,
    };
    let scott = CardAsset {
        asset_id: 31_003,
        app_id: 960_910,
        name: "Scott".into(),
        market_hash_name: "960910-Scott (Foil)".into(),
        foil: true,
        marketable: true,
        tradable: true,
    };
    assert_eq!(
        cards,
        [madison(31_001), madison(31_002), scott],
        "each copy on its own, and the emoticon left out"
    );
    assert!(
        site.received_requests().await.unwrap().is_empty(),
        "asked over the CM connection, not of the site"
    );
}
