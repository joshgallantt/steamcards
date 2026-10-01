//! What cards are worth, worked out from the prices known: pure functions,
//! so every figure on screen can be checked by hand. Fees are worked out
//! card by card, and then summed, never on a total: they're rounded and
//! have a minimum, so the two differ (research §1.4).

use chrono::{DateTime, Utc};
use game::{AppId, SteamLibrary};
use money::Money;

use crate::{Basis, Estimate, Held, HeldCard, Price, PriceBook, SetPrices, Wallet};

/// What a card with this price is worth on `basis`, in the wallet's
/// currency: its lowest listing (list), or what that pays the seller (net).
/// `None` when that isn't known, or is in another currency: money in
/// different currencies is never added or converted.
pub fn value_of(price: &Price, basis: Basis, wallet: &Wallet) -> Option<Money> {
    let Price::Known(quote) = price else {
        return None;
    };
    let buyer_pays = quote.ask?;
    if buyer_pays.currency != wallet.currency {
        return None;
    }
    let minor = match basis {
        Basis::List => buyer_pays.minor,
        Basis::Net => wallet.seller_gets(buyer_pays.minor),
    };
    Some(Money::new(minor, wallet.currency))
}

/// What `cards` are worth on `basis`, each copy at its own price: two drops
/// of one card count twice. A card that isn't priced never counts as
/// nothing: it's counted as unpriced, and so are `unidentified` drops, whose
/// card isn't known, whether it's still being found out or nothing could
/// tell. A price over 6 hours old still counts, and the total says how old
/// the oldest is.
pub fn held_value(
    cards: &[HeldCard],
    unidentified: u32,
    book: &PriceBook,
    basis: Basis,
    wallet: &Wallet,
    now: DateTime<Utc>,
) -> Held {
    let mut held = Held {
        total: Money::zero(wallet.currency),
        priced: 0,
        unpriced: unidentified,
        not_marketable: 0,
        oldest: None,
    };
    for card in cards {
        let price = book.price(card);
        if price == Price::NotMarketable {
            held.not_marketable += 1;
            continue;
        }
        let total = value_of(&price, basis, wallet).and_then(|v| held.total.checked_add(v));
        let Some(total) = total else {
            held.unpriced += 1;
            continue;
        };
        held.total = total;
        held.priced += 1;
        if let Price::Known(quote) = &price
            && quote.is_stale(now)
        {
            held.oldest = held.oldest.max(Some(quote.age(now)));
        }
    }
    held
}

/// What a drop from this set is likely worth on `basis`: each normal card
/// valued first, then the mean of those that have a value, to the nearest
/// hundredth. Drops are taken to be spread evenly over the set (research
/// §3.3). Foils are left out until how often one drops is known, so
/// estimates built on this are "excl. foils".
pub fn expected_per_drop(set: &SetPrices, basis: Basis, wallet: &Wallet) -> Option<Money> {
    let (sum, n) = normal_values(set, basis, wallet)?;
    Some(Money::new(nearest(sum, n)?, wallet.currency))
}

/// What the cards still to drop in the games farmed are likely worth: each
/// game's drops left times what a drop of it is worth, rounded once for the
/// game, not for each drop. `order` is the farm order, as farming's own
/// forecast takes it: a game not in it (skipped, a sale's badge, not a
/// priority with "only priority" on) isn't farmed, so drops nothing. Games
/// whose sets aren't priced are left out, and counted.
pub fn value_left(
    library: &SteamLibrary,
    order: &[AppId],
    book: &PriceBook,
    basis: Basis,
    wallet: &Wallet,
) -> Estimate {
    let mut left = Estimate {
        value: Money::zero(wallet.currency),
        excl_foils: true,
        unpriced_games: 0,
    };
    let farmed = order
        .iter()
        .filter_map(|&id| library.game(id))
        .filter(|g| g.has_drops_left());
    for game in farmed {
        let value = book
            .sets
            .get(&game.app_id)
            .and_then(|set| normal_values(set, basis, wallet))
            .and_then(|(sum, n)| nearest(sum.checked_mul(i64::from(game.drops.remaining))?, n))
            .and_then(|minor| left.value.checked_add(Money::new(minor, wallet.currency)));
        match value {
            Some(value) => left.value = value,
            None => left.unpriced_games += 1,
        }
    }
    left
}

/// A set's normal cards that have a value on `basis`, each valued on its
/// own: the sum, and how many there are.
fn normal_values(set: &SetPrices, basis: Basis, wallet: &Wallet) -> Option<(i64, i64)> {
    let values: Vec<i64> = set
        .normal
        .iter()
        .filter_map(|card| value_of(&card.price, basis, wallet))
        .map(|value| value.minor)
        .collect();
    let n = i64::try_from(values.len()).ok().filter(|&n| n > 0)?;
    let sum = values.iter().try_fold(0i64, |sum, v| sum.checked_add(*v))?;
    Some((sum, n))
}

/// `sum / n` to the nearest whole hundredth, halves up.
fn nearest(sum: i64, n: i64) -> Option<i64> {
    Some((sum.checked_mul(2)? + n).div_euclid(2 * n))
}

/// The value once every card has dropped: what's held now, and what's still
/// to drop (research §3.3). Both come from the same wallet, so they're in
/// one currency; were they not, only what's still to drop could be told.
pub fn on_completion(held: &Held, left: &Estimate) -> Estimate {
    Estimate {
        value: held.total.checked_add(left.value).unwrap_or(left.value),
        ..*left
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use card::CardKind;
    use chrono::TimeDelta;
    use game::{AppId, CardDrops, Game};

    use super::*;
    use money::Currency;

    use crate::{PriceQuote, PricedCard};

    fn noon() -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2026-09-29T12:00:00Z")
            .unwrap()
            .to_utc()
    }

    fn pounds() -> Wallet {
        Wallet::new(Currency::GBP)
    }

    fn quote(ask: Option<i64>) -> Price {
        quote_in(Currency::GBP, ask)
    }

    fn quote_in(currency: Currency, ask: Option<i64>) -> Price {
        Price::Known(PriceQuote {
            ask: ask.map(|a| Money::new(a, currency)),
            ask_depth: ask.map(|_| 5),
            fetched_at: noon(),
        })
    }

    fn set(app_id: u32, asks: &[i64]) -> SetPrices {
        set_in(Currency::GBP, app_id, asks)
    }

    fn set_in(currency: Currency, app_id: u32, asks: &[i64]) -> SetPrices {
        SetPrices {
            app_id: AppId(app_id),
            normal: asks
                .iter()
                .enumerate()
                .map(|(i, &ask)| PricedCard {
                    name: format!("Card {i}"),
                    market_hash_name: format!("{app_id}-Card {i}"),
                    price: quote_in(currency, Some(ask)),
                })
                .collect(),
            foil: Vec::new(),
            fetched_at: noon(),
            retry_at: None,
        }
    }

    fn game(app_id: u32, remaining: u32) -> Game {
        Game {
            app_id: AppId(app_id),
            name: format!("Game {app_id}"),
            hours: 5.0,
            drops: CardDrops {
                received: 1,
                remaining,
            },
            badge_level: 0,
        }
    }

    #[test]
    fn a_card_is_worth_its_listing_or_what_that_pays() {
        let wallet = pounds();
        let gbp = |minor| Some(Money::new(minor, Currency::GBP));
        // G-Man, 2026-09-29: an ask of 11p (research §1.4).
        let listed = quote(Some(11));
        assert_eq!(value_of(&listed, Basis::List, &wallet), gbp(11));
        assert_eq!(value_of(&listed, Basis::Net, &wallet), gbp(9));
        assert_eq!(value_of(&quote(None), Basis::List, &wallet), None);
        assert_eq!(value_of(&Price::Pending, Basis::List, &wallet), None);
        assert_eq!(value_of(&Price::NoMarket, Basis::List, &wallet), None);
    }

    #[test]
    fn a_price_in_another_currency_is_never_converted() {
        let dollars = Price::Known(PriceQuote {
            ask: Some(Money::new(7, Currency::USD)),
            ask_depth: Some(3),
            fetched_at: noon(),
        });
        assert_eq!(value_of(&dollars, Basis::List, &pounds()), None);
        assert_eq!(
            value_of(&dollars, Basis::List, &Wallet::new(Currency::USD)),
            Some(Money::new(7, Currency::USD))
        );
    }

    #[test]
    fn fees_come_off_each_card_before_the_mean() {
        // Half-Life 2's normal cards, 2026-09-29: 13.75¢ on average, 11.75¢
        // after fees card by card, where fees off the mean would say 12¢.
        let hl2 = set_in(Currency::USD, 220, &[13, 13, 14, 14, 12, 14, 16, 14]);
        let dollars = Wallet::new(Currency::USD);
        let each = |basis| expected_per_drop(&hl2, basis, &dollars).map(|m| m.minor);
        assert_eq!(each(Basis::List), Some(14), "13.75¢ to the nearest cent");
        assert_eq!(each(Basis::Net), Some(12), "11.75¢");

        let sets_with_halves = set(1, &[4, 5]);
        assert_eq!(
            expected_per_drop(&sets_with_halves, Basis::List, &pounds()).map(|m| m.minor),
            Some(5),
            "4.5p rounds up"
        );
        assert_eq!(
            expected_per_drop(&set(2, &[]), Basis::List, &pounds()),
            None
        );
    }

    #[test]
    fn a_mean_leaves_out_cards_without_a_value() {
        let mut heavy_rain = set(960_910, &[5, 4, 5, 6]);
        heavy_rain.normal[3].price = Price::NoMarket;
        assert_eq!(
            expected_per_drop(&heavy_rain, Basis::List, &pounds()).map(|m| m.minor),
            Some(5),
            "(5 + 4 + 5) / 3"
        );
    }

    #[test]
    fn whats_left_to_drop_is_each_games_drops_left_times_a_drop() {
        let library = SteamLibrary::new(vec![
            game(10, 3),
            game(20, 2),
            game(30, 4),
            game(40, 0),
            game(50, 2),
        ]);
        let mut book = PriceBook::default();
        book.sets.insert(AppId(10), set(10, &[5, 5]));
        book.sets.insert(AppId(20), set(20, &[8]));
        book.sets.insert(AppId(40), set(40, &[100]));
        // Game 50 is skipped: not in the farm order.
        let order = [10, 20, 30, 40].map(AppId);

        let left = value_left(&library, &order, &book, Basis::List, &pounds());

        assert_eq!(left.value, Money::new(3 * 5 + 2 * 8, Currency::GBP));
        assert_eq!(
            left.unpriced_games, 1,
            "game 30 isn't priced; game 50, never farmed, never is"
        );
        assert!(left.excl_foils);
        let net = value_left(&library, &order, &book, Basis::Net, &pounds());
        assert_eq!(
            net.value,
            Money::new(3 * 3 + 2 * 6, Currency::GBP),
            "after fees"
        );
    }

    #[test]
    fn whats_left_is_rounded_once_for_each_game_not_for_each_drop() {
        // Half-Life 2's net values, 11.75¢ a drop: 4 drops are 47¢, and 10
        // are 117.5¢, not 4 or 10 of a drop rounded to 12¢.
        let hl2 = set_in(Currency::USD, 220, &[13, 13, 14, 14, 12, 14, 16, 14]);
        let dollars = Wallet::new(Currency::USD);
        let mut book = PriceBook::default();
        book.sets.insert(AppId(220), hl2);
        let left = |remaining| {
            value_left(
                &SteamLibrary::new(vec![game(220, remaining)]),
                &[AppId(220)],
                &book,
                Basis::Net,
                &dollars,
            )
            .value
            .minor
        };
        assert_eq!(left(4), 47);
        assert_eq!(left(10), 118, "117.5¢, halves up");
    }

    #[test]
    fn stale_prices_count_and_the_oldest_is_told() {
        let mut book = PriceBook::default();
        book.sets.insert(AppId(10), set(10, &[5]));
        let mut fresh = set(20, &[8]);
        fresh.fetched_at = noon() + TimeDelta::hours(7);
        if let Price::Known(q) = &mut fresh.normal[0].price {
            q.fetched_at = noon() + TimeDelta::hours(7);
        }
        book.sets.insert(AppId(20), fresh);
        let cards = [
            HeldCard::named(AppId(10), "Card 0", CardKind::Normal),
            HeldCard::named(AppId(20), "Card 0", CardKind::Normal),
        ];

        let at = |hours| noon() + TimeDelta::hours(hours);
        let held = held_value(&cards, 0, &book, Basis::List, &pounds(), at(8));
        assert_eq!(held.total, Money::new(13, Currency::GBP));
        assert_eq!(held.oldest, Some(Duration::from_secs(8 * 60 * 60)));
        let early = held_value(&cards, 0, &book, Basis::List, &pounds(), at(6));
        assert_eq!(early.oldest, None, "6 hours old is still fresh");
    }

    #[test]
    fn completion_adds_whats_held_to_whats_left() {
        let held = Held {
            total: Money::new(145, Currency::GBP),
            priced: 13,
            unpriced: 3,
            not_marketable: 0,
            oldest: None,
        };
        let left = Estimate {
            value: Money::new(1_534, Currency::GBP),
            excl_foils: true,
            unpriced_games: 2,
        };
        assert_eq!(
            on_completion(&held, &left),
            Estimate {
                value: Money::new(1_679, Currency::GBP),
                ..left
            },
            "£1.45 + £15.34 = £16.79"
        );
    }
}
