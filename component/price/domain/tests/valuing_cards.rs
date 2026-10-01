//! Unit tier: what cards are worth, as the user reads it on screen, worked
//! out by the valuations alone.
//! The session is the design's own (docs/design/ui.md, the haul at 17:31):
//! 16 cards from 6 games, a foil among them, two of them second copies, and
//! three that can't be counted yet.

use card::{CardKind, test_support::card_asset};
use chrono::{DateTime, TimeDelta, Utc};
use money::{Currency, Money};
use price::{
    Basis, HeldCard, Offers, Price, PriceBook, SetPrices, expected_per_drop, held_value,
    on_completion,
    test_support::{order_book, pounds, session_start, set_prices},
    value_left,
};
use steam_library::{AppId, SteamLibrary, test_support::game};

const HOLLOW_KNIGHT: u32 = 367_520;
const INSCRYPTION: u32 = 1_092_790;
const HADES: u32 = 1_145_360;
const CELESTE: u32 = 504_230;
const GOROGOA: u32 = 557_600;
const HEAVY_RAIN: u32 = 960_910;

/// 17:31, eight hours and seventeen minutes into the session.
fn now() -> DateTime<Utc> {
    session_start() + TimeDelta::minutes(8 * 60 + 17)
}

fn pence(minor: i64) -> Money {
    Money::new(minor, Currency::GBP)
}

/// The prices known at 17:31. Celeste's were looked up at 09:21, and its
/// latest lookup failed: Badeline's price from then still counts, and
/// Madeline, which wasn't listed then, can't be told. Nobody sells The
/// Fruit.
fn the_book() -> PriceBook {
    let fresh = now() - TimeDelta::minutes(30);
    let mut celeste = set_prices(
        CELESTE,
        &[("Badeline", 6)],
        &[],
        session_start() + TimeDelta::minutes(7),
    );
    celeste.retry_at = Some(now() + TimeDelta::hours(20));
    let sets: [SetPrices; 6] = [
        set_prices(
            HOLLOW_KNIGHT,
            &[("Hornet", 9), ("Zote", 7), ("The Knight", 11)],
            &[],
            fresh,
        ),
        set_prices(
            INSCRYPTION,
            &[("Leshy", 6), ("Stoat", 5), ("Stinkbug", 5)],
            &[],
            fresh,
        ),
        set_prices(
            HADES,
            &[("Zagreus", 8), ("Nyx", 9)],
            &[("Thanatos", 62)],
            fresh,
        ),
        celeste,
        set_prices(GOROGOA, &[("The Boy", 4)], &[], fresh),
        set_prices(
            HEAVY_RAIN,
            &[
                ("Ethan", 5),
                ("Carter", 4),
                ("Madison", 5),
                ("Norman", 6),
                ("Scott", 4),
            ],
            &[],
            fresh,
        ),
    ];
    let mut book = PriceBook::default();
    for set in sets {
        book.sets.insert(set.app_id, set);
    }
    // Each held card's order book, looked up as it dropped: what selling it
    // now pays, from each best offer.
    let offers = [
        ("367520-Hornet", 9, 7),
        ("367520-Zote", 7, 5),
        ("367520-The Knight", 11, 9),
        ("1092790-Leshy", 6, 4),
        ("1092790-Stoat", 5, 3),
        ("1092790-Stinkbug", 5, 3),
        ("1145360-Zagreus", 8, 6),
        ("1145360-Thanatos (Foil)", 62, 41),
        ("1145360-Nyx", 9, 7),
        ("504230-Badeline", 6, 4),
        ("557600-The Boy", 4, 3),
        ("960910-Madison", 5, 4),
    ];
    let looked_up_at = now() - TimeDelta::minutes(5);
    for (hash, ask, bid) in offers {
        book.offers.insert(
            hash.into(),
            Offers {
                price: order_book(ask, bid, looked_up_at),
                looked_up_at,
            },
        );
    }
    book.offers.insert(
        "557600-The Fruit".into(),
        Offers {
            price: Price::NoMarket,
            looked_up_at,
        },
    );
    book
}

/// Every copy that dropped this session, and the one still being
/// identified.
fn the_haul() -> (Vec<HeldCard>, u32) {
    let mut thanatos = card_asset(14_02, HADES, "Thanatos");
    thanatos.kind = CardKind::Foil;
    thanatos.market_hash_name = "1145360-Thanatos (Foil)".into();
    let assets = [
        card_asset(9_44, HOLLOW_KNIGHT, "Hornet"),
        card_asset(10_12, HOLLOW_KNIGHT, "Zote"),
        card_asset(10_41, HOLLOW_KNIGHT, "The Knight"),
        card_asset(11_15, INSCRYPTION, "Leshy"),
        card_asset(11_43, INSCRYPTION, "Stoat"),
        card_asset(12_20, INSCRYPTION, "Stinkbug"),
        card_asset(12_58, HADES, "Zagreus"),
        card_asset(13_30, HADES, "Zagreus"),
        thanatos,
        card_asset(14_35, HADES, "Nyx"),
        card_asset(15_09, CELESTE, "Madeline"),
        card_asset(15_41, CELESTE, "Badeline"),
        card_asset(16_15, GOROGOA, "The Boy"),
        card_asset(16_48, GOROGOA, "The Fruit"),
        card_asset(17_05, HEAVY_RAIN, "Madison"),
    ];
    (assets.iter().map(HeldCard::from).collect(), 1)
}

#[test]
fn this_sessions_cards_are_worth_at_least_their_prices_and_say_whats_missing() {
    let (cards, identifying) = the_haul();
    let book = the_book();
    let at = |basis| held_value(&cards, identifying, &book, basis, &pounds(), now());

    let list = at(Basis::List);
    assert_eq!(list.total.to_string(), "£1.45", "≥ £1.45 this session");
    assert_eq!(list.priced, 13);
    assert_eq!(
        list.unpriced, 3,
        "Madeline can't be told, nobody sells The Fruit, and one is being identified"
    );
    assert_eq!(list.oldest.map(|d| d.as_secs() / 60), Some(8 * 60 + 10));

    assert_eq!(
        at(Basis::Net).total.to_string(),
        "£1.14",
        "≥ £1.14 after fees"
    );
    let sold_now = at(Basis::Instant);
    assert_eq!(sold_now.total.to_string(), "£0.74", "≥ £0.74 if sold now");
    assert_eq!(sold_now.unpriced, 3);
    assert_eq!(sold_now.oldest, None, "the offers are fresh");
}

#[test]
fn a_second_copy_counts_at_its_own_price() {
    let book = the_book();
    let zagreus = HeldCard::from(&card_asset(12_58, HADES, "Zagreus"));
    let twice = [zagreus.clone(), zagreus];

    let held = held_value(&twice, 0, &book, Basis::List, &pounds(), now());

    assert_eq!(held.total, pence(16), "two drops of one card are two drops");
    assert_eq!(held.priced, 2);
}

#[test]
fn a_card_that_isnt_priced_never_counts_as_nothing() {
    let book = PriceBook::default();
    let (cards, identifying) = the_haul();

    let held = held_value(&cards, identifying, &book, Basis::List, &pounds(), now());

    assert_eq!(held.priced, 0, "nothing priced yet");
    assert_eq!(held.unpriced, 16);
    assert_eq!(
        held.total,
        pence(0),
        "a total of nothing counted, not a value"
    );
}

#[test]
fn a_card_known_only_by_name_is_priced_from_its_set() {
    let book = the_book();
    let by_name = HeldCard::named(AppId(HEAVY_RAIN), "Madison", CardKind::Normal);

    assert_eq!(
        held_value(
            std::slice::from_ref(&by_name),
            0,
            &book,
            Basis::List,
            &pounds(),
            now()
        )
        .total,
        pence(5)
    );
    assert_eq!(
        held_value(&[by_name], 0, &book, Basis::Instant, &pounds(), now()).total,
        pence(2),
        "its offers, by the hash name its set gives it"
    );
}

#[test]
fn a_card_that_cant_be_sold_is_left_out() {
    let mut badge_reward = card_asset(1, HEAVY_RAIN, "Madison");
    badge_reward.marketable = false;

    let held = held_value(
        &[HeldCard::from(&badge_reward)],
        0,
        &the_book(),
        Basis::List,
        &pounds(),
        now(),
    );

    assert_eq!((held.priced, held.unpriced, held.not_marketable), (0, 0, 1));
}

#[test]
fn a_price_in_another_currency_is_shown_but_left_out_of_totals() {
    let mut book = the_book();
    let dollars = set_prices(GOROGOA, &[("The Boy", 5)], &[], now());
    book.sets.insert(
        AppId(GOROGOA),
        SetPrices {
            normal: dollars
                .normal
                .into_iter()
                .map(|mut card| {
                    if let Price::Known(quote) = &mut card.price {
                        quote.ask = Some(Money::new(7, Currency::USD));
                    }
                    card
                })
                .collect(),
            ..dollars
        },
    );
    let the_boy = HeldCard::named(AppId(GOROGOA), "The Boy", CardKind::Normal);

    let Price::Known(quote) = book.price(&the_boy, Basis::List) else {
        panic!("it has a price");
    };
    assert_eq!(quote.ask.unwrap().to_string(), "$0.07");
    let held = held_value(&[the_boy], 0, &book, Basis::List, &pounds(), now());
    assert_eq!((held.priced, held.unpriced), (0, 1), "never converted");
}

#[test]
fn heavy_rains_next_drop_is_worth_about_five_pence() {
    let book = the_book();
    let set = &book.sets[&AppId(HEAVY_RAIN)];

    let each = expected_per_drop(set, Basis::List, &pounds());

    assert_eq!(each, Some(pence(5)), "the mean of 5, 4, 5, 6 and 4 is 4.8p");
    assert_eq!(
        expected_per_drop(set, Basis::Net, &pounds()),
        Some(pence(3)),
        "3, 2, 3, 4 and 2 after fees, card by card"
    );
}

#[test]
fn whats_still_to_drop_is_estimated_without_foils_and_says_whats_missing() {
    let mut book = the_book();
    let at = now();
    book.sets.insert(
        AppId(367_380),
        set_prices(
            367_380,
            &[
                ("Boy", 4),
                ("Lever", 5),
                ("Spider", 6),
                ("Hotel", 9),
                ("Cave", 6),
            ],
            &[("Boy", 49)],
            at,
        ),
    );
    book.sets.insert(
        AppId(457_140),
        set_prices(
            457_140,
            &[
                ("Duplicant", 5),
                ("Hatch", 6),
                ("Slickster", 7),
                ("Pip", 10),
                ("Shine Bug", 7),
            ],
            &[],
            at,
        ),
    );
    let mut heavy_rain = game(HEAVY_RAIN, 4.0, 3, 1);
    heavy_rain.name = "Heavy Rain".into();
    let library = SteamLibrary::new(vec![
        heavy_rain,
        game(367_380, 3.4, 3, 2),
        game(457_140, 3.4, 3, 2),
        game(1_966_720, 0.0, 0, 5),
        game(HADES, 12.0, 4, 0),
    ]);

    let order = [HEAVY_RAIN, 367_380, 457_140, 1_966_720].map(AppId);

    let left = value_left(&library, &order, &book, Basis::List, &pounds());

    assert_eq!(
        left.value.to_string(),
        "£0.31",
        "≈ £0.05 for Heavy Rain, £0.12 for LIMBO, £0.14 for Oxygen Not Included"
    );
    assert!(left.excl_foils, "LIMBO's foil isn't in it");
    assert_eq!(left.unpriced_games, 1, "one game isn't priced yet");
    assert_eq!(left.basis, Basis::List);

    let (cards, identifying) = the_haul();
    let held = held_value(&cards, identifying, &book, Basis::List, &pounds(), now());
    let done = on_completion(&held, &left);
    assert_eq!(
        done.value.to_string(),
        "£1.76",
        "£1.45 held and £0.31 to come"
    );
    assert_eq!(done.unpriced_games, 1);
    assert!(done.excl_foils);
}

#[test]
fn on_the_instant_basis_cards_still_to_drop_stay_after_fees() {
    let book = the_book();
    let library = SteamLibrary::new(vec![game(HEAVY_RAIN, 4.0, 3, 1)]);

    let left = value_left(
        &library,
        &[AppId(HEAVY_RAIN)],
        &book,
        Basis::Instant,
        &pounds(),
    );

    assert_eq!(left.basis, Basis::Net, "and the estimate says so");
    assert_eq!(
        left.value,
        pence(3),
        "no offers to sell a card to before it drops"
    );
}
