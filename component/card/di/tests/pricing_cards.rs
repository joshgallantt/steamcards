//! Acceptance tier: prices as the user meets them, through the card
//! component as steamcards wires it, against stand-ins for Steam and its
//! market. The hours-long runs, on paused time, are in the card domain's
//! own tests.

// Each test file takes only the support it uses: what it left out would be
// dead code in it.
mod support {
    pub(crate) mod market_player;
}

use card::{CardKind, Price, PriceEvent};
use game::AppId;
use money::{Currency, Money};
use support::market_player::Player;
use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{method, path, query_param},
};

/// `search/render`'s answer for a game's cards of one border: `(name, pence,
/// text, listings, type)` each.
fn listing(app_id: u32, cards: &[(&str, i64, &str, u32, &str)]) -> ResponseTemplate {
    let results: Vec<_> = cards
        .iter()
        .map(|&(name, price, text, listings, kind)| {
            serde_json::json!({
                "name": name,
                "hash_name": format!("{app_id}-{name}"),
                "sell_listings": listings,
                "sell_price": price,
                "sell_price_text": text,
                "asset_description": { "type": kind },
            })
        })
        .collect();
    ResponseTemplate::new(200).set_body_json(serde_json::json!({
        "success": true,
        "start": 0,
        "pagesize": 10,
        "total_count": cards.len(),
        "results": results,
    }))
}

async fn lists(site: &MockServer, app_id: u32, border: u8, answer: ResponseTemplate) {
    Mock::given(method("GET"))
        .and(path("/market/search/render/"))
        .and(query_param(
            "category_753_Game[]",
            format!("tag_app_{app_id}"),
        ))
        .and(query_param(
            "category_753_cardborder[]",
            format!("tag_cardborder_{border}"),
        ))
        .respond_with(answer)
        .mount(site)
        .await;
}

fn ask(price: &Price) -> Option<Money> {
    match price {
        Price::Known(quote) => quote.ask,
        _ => None,
    }
}

#[tokio::test]
async fn a_sets_prices_are_shown_once_looked_up_and_kept() {
    let player = Player::new("priced").await;
    lists(
        &player.site,
        960_910,
        0,
        listing(
            960_910,
            &[("Madison", 5, "£0.05", 1_204, "Heavy Rain Trading Card")],
        ),
    )
    .await;
    lists(
        &player.site,
        960_910,
        1,
        listing(
            960_910,
            &[(
                "Madison (Foil)",
                60,
                "£0.60",
                4,
                "Heavy Rain Foil Trading Card",
            )],
        ),
    )
    .await;

    let event = player.has_prices_looked_up(&[960_910]).await;

    assert!(matches!(event, PriceEvent::AllPriced { games: 1, .. }));
    let pence = |pence| Some(Money::new(pence, Currency::GBP));
    let heavy_rain = &player.sees().sets[&AppId(960_910)];
    assert_eq!(
        ask(&heavy_rain.price("Madison", CardKind::Normal)),
        pence(5)
    );
    assert_eq!(ask(&heavy_rain.price("Madison", CardKind::Foil)), pence(60));

    let player = player.comes_back();
    assert_eq!(
        ask(&player.sees().sets[&AppId(960_910)].price("Madison", CardKind::Normal)),
        pence(5),
        "kept when steamcards starts again"
    );
}
