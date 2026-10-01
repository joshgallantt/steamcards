//! Acceptance tier: a game's cards, and which card each new item is, as the
//! user meets them, through the card component as steamcards wires it,
//! against stand-ins for Steam and its site.

// Each test file takes only the support it uses: what it left out would be
// dead code in it.
mod support {
    pub(crate) mod player;
}

use card::{AssetId, Card, CardAsset, CardError, CardKind};
use game::AppId;
use steam_api::{EResult, test_support::HeldItem};
use support::player::Player;

#[tokio::test]
async fn a_games_cards_are_looked_at_on_its_own_page() {
    let player = Player::new("cards").await;
    player.has_counter_strikes_card_page(CardKind::Normal).await;

    let cs = player.looks_at_cards(730).await.unwrap();

    assert_eq!(cs.game.drops.remaining, 2);
    assert_eq!(cs.set.len(), 5);
    assert_eq!(
        cs.set.cards()[0],
        Card {
            name: "Anarchist".into(),
            owned: 2,
        }
    );
    assert_eq!(cs.set.collected(), 2);
}

#[tokio::test]
async fn a_games_foils_are_counted_on_a_page_of_their_own() {
    let player = Player::new("foils").await;
    player.has_counter_strikes_card_page(CardKind::Foil).await;

    let foils = player.looks_at_foils(730).await.unwrap();
    assert_eq!(foils.kind(), CardKind::Foil);

    let held: Vec<(&str, u32)> = foils
        .cards()
        .iter()
        .map(|c| (c.name.as_str(), c.owned))
        .collect();
    assert_eq!(held[..3], [("Anarchist", 2), ("Balkan", 0), ("FBI", 1)]);
}

#[tokio::test]
async fn new_items_are_told_apart_as_the_cards_they_are() {
    let player = Player::new("items").await;
    player.steam.hold(vec![
        HeldItem::card(31_001, 960_910, "Madison"),
        HeldItem::card(31_002, 960_910, "Madison"),
        HeldItem::foil(31_003, 960_910, "Scott (Foil)"),
        HeldItem::other(31_004, 960_910, 4, ":origami:"),
    ]);

    let cards = player
        .identifies(&[31_001, 31_002, 31_003, 31_004])
        .await
        .unwrap();

    let names: Vec<(AssetId, &str, CardKind)> = cards
        .iter()
        .map(|c: &CardAsset| (c.asset_id, c.name.as_str(), c.kind))
        .collect();
    assert_eq!(
        names,
        [
            (AssetId(31_001), "Madison", CardKind::Normal),
            (AssetId(31_002), "Madison", CardKind::Normal),
            (AssetId(31_003), "Scott", CardKind::Foil),
        ],
        "each copy on its own, and the emoticon left out"
    );
    assert!(cards.iter().all(|c| c.app_id == AppId(960_910)));
    assert!(
        player.site.received_requests().await.unwrap().is_empty(),
        "asked of Steam, not of the site"
    );
}

#[tokio::test]
async fn items_steam_wont_describe_say_why() {
    let player = Player::new("refused").await;
    player.steam.refuse_inventory(1, EResult::ACCESS_DENIED);

    let told = player.identifies(&[31_001]).await;

    assert!(matches!(told, Err(CardError::Unavailable(_))), "{told:?}");
}
