//! What cards are worth, worked out from the prices known: pure functions,
//! so every figure on screen can be checked by hand. Fees are worked out
//! card by card, and then summed, never on a total: they're rounded and
//! have a minimum, so the two differ (research §1.4).

use chrono::{DateTime, Utc};
use library::SteamLibrary;

use crate::{
    Basis, Estimate, Held, HeldCard, Money, Price, PriceBook, QuoteSource, SetPrices, Wallet,
};

/// What a card with this price is worth on `basis`, in the wallet's
/// currency: its lowest listing (list), what that pays the seller (net), or
/// what its best offer pays the seller (instant, from an order book only).
/// `None` when that isn't known, or is in another currency: money in
/// different currencies is never added or converted.
pub fn value_of(price: &Price, basis: Basis, wallet: &Wallet) -> Option<Money> {
    let Price::Known(quote) = price else {
        return None;
    };
    let buyer_pays = match basis {
        Basis::List | Basis::Net => quote.ask?,
        Basis::Instant if quote.source == QuoteSource::OrderBook => quote.bid?,
        Basis::Instant => return None,
    };
    if buyer_pays.currency != wallet.currency {
        return None;
    }
    let minor = match basis {
        Basis::List => buyer_pays.minor,
        Basis::Net | Basis::Instant => wallet.seller_gets(buyer_pays.minor),
    };
    Some(Money::new(minor, wallet.currency))
}

/// What `cards` are worth on `basis`, each copy at its own price: two drops
/// of one card count twice. A card that isn't priced never counts as
/// nothing: it's counted as unpriced, and so are `unidentified` drops, not
/// known yet. A price over 6 hours old still counts, and the total says how
/// old the oldest is.
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
        let price = book.price(card, basis);
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
/// estimates built on this are "excl. foils". On the instant basis it's the
/// value after fees (see [`Basis::still_to_drop`]).
pub fn expected_per_drop(set: &SetPrices, basis: Basis, wallet: &Wallet) -> Option<Money> {
    let basis = basis.still_to_drop();
    let values: Vec<i64> = set
        .normal
        .iter()
        .filter_map(|card| value_of(&card.price, basis, wallet))
        .map(|value| value.minor)
        .collect();
    let n = i64::try_from(values.len()).ok().filter(|&n| n > 0)?;
    let sum = values.iter().try_fold(0i64, |sum, v| sum.checked_add(*v))?;
    // The nearest hundredth, halves up.
    let mean = (sum.checked_mul(2)? + n).div_euclid(2 * n);
    Some(Money::new(mean, wallet.currency))
}

/// What the cards still to drop across the library are likely worth: each
/// game's drops left times what a drop of it is worth. Games whose sets
/// aren't priced are left out, and counted. On the instant basis, the cards
/// still to drop are valued after fees, and the estimate says so.
pub fn value_left(
    library: &SteamLibrary,
    book: &PriceBook,
    basis: Basis,
    wallet: &Wallet,
) -> Estimate {
    let mut left = Estimate {
        value: Money::zero(wallet.currency),
        excl_foils: true,
        unpriced_games: 0,
        basis: basis.still_to_drop(),
    };
    for game in library.with_drops_left() {
        let value = book
            .sets
            .get(&game.app_id)
            .and_then(|set| expected_per_drop(set, basis, wallet))
            .and_then(|each| each.times(game.drops.remaining))
            .and_then(|value| left.value.checked_add(value));
        match value {
            Some(value) => left.value = value,
            None => left.unpriced_games += 1,
        }
    }
    left
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

    use chrono::TimeDelta;
    use library::{CardDrops, Game};

    use super::*;
    use crate::{Currency, PriceQuote, PricedCard};

    fn noon() -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2026-09-29T12:00:00Z")
            .unwrap()
            .to_utc()
    }

    fn pounds() -> Wallet {
        Wallet::new(Currency::GBP)
    }

    fn quote(ask: Option<i64>, bid: Option<i64>, source: QuoteSource) -> Price {
        quote_in(Currency::GBP, ask, bid, source)
    }

    fn quote_in(
        currency: Currency,
        ask: Option<i64>,
        bid: Option<i64>,
        source: QuoteSource,
    ) -> Price {
        Price::Known(PriceQuote {
            ask: ask.map(|a| Money::new(a, currency)),
            bid: bid.map(|b| Money::new(b, currency)),
            ask_depth: ask.map(|_| 5),
            bid_depth: bid.map(|_| 5),
            source,
            fetched_at: noon(),
        })
    }

    fn set(app_id: u32, asks: &[i64]) -> SetPrices {
        set_in(Currency::GBP, app_id, asks)
    }

    fn set_in(currency: Currency, app_id: u32, asks: &[i64]) -> SetPrices {
        SetPrices {
            app_id,
            normal: asks
                .iter()
                .enumerate()
                .map(|(i, &ask)| PricedCard {
                    name: format!("Card {i}"),
                    market_hash_name: format!("{app_id}-Card {i}"),
                    price: quote_in(currency, Some(ask), None, QuoteSource::Search),
                })
                .collect(),
            foil: Vec::new(),
            fetched_at: noon(),
            retry_at: None,
        }
    }

    fn game(app_id: u32, remaining: u32) -> Game {
        Game {
            app_id,
            name: format!("Game {app_id}"),
            hours: 5.0,
            drops: CardDrops {
                received: 1,
                remaining,
            },
            badge_level: 0,
            cards: Vec::new(),
        }
    }

    #[test]
    fn a_card_is_worth_its_listing_what_that_pays_or_what_an_offer_pays() {
        let wallet = pounds();
        let gbp = |minor| Some(Money::new(minor, Currency::GBP));
        // G-Man, 2026-09-29: an ask of 11p and a bid of 8p (research §1.4).
        let book = quote(Some(11), Some(8), QuoteSource::OrderBook);
        assert_eq!(value_of(&book, Basis::List, &wallet), gbp(11));
        assert_eq!(value_of(&book, Basis::Net, &wallet), gbp(9));
        assert_eq!(value_of(&book, Basis::Instant, &wallet), gbp(6));

        let search = quote(Some(11), None, QuoteSource::Search);
        assert_eq!(
            value_of(&search, Basis::Instant, &wallet),
            None,
            "no offers"
        );
        let no_offers = quote(Some(11), None, QuoteSource::OrderBook);
        assert_eq!(value_of(&no_offers, Basis::Instant, &wallet), None);
        assert_eq!(value_of(&Price::Pending, Basis::List, &wallet), None);
        assert_eq!(value_of(&Price::NoMarket, Basis::List, &wallet), None);
    }

    #[test]
    fn a_price_in_another_currency_is_never_converted() {
        let dollars = Price::Known(PriceQuote {
            ask: Some(Money::new(7, Currency::USD)),
            bid: None,
            ask_depth: Some(3),
            bid_depth: None,
            source: QuoteSource::Search,
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
        assert_eq!(each(Basis::Instant), Some(12), "after fees");

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
        let library = SteamLibrary::new(vec![game(10, 3), game(20, 2), game(30, 4), game(40, 0)]);
        let mut book = PriceBook::default();
        book.sets.insert(10, set(10, &[5, 5]));
        book.sets.insert(20, set(20, &[8]));
        book.sets.insert(40, set(40, &[100]));

        let left = value_left(&library, &book, Basis::List, &pounds());

        assert_eq!(left.value, Money::new(3 * 5 + 2 * 8, Currency::GBP));
        assert_eq!(left.unpriced_games, 1, "game 30 isn't priced");
        assert!(left.excl_foils);
        assert_eq!(left.basis, Basis::List);
        let instant = value_left(&library, &book, Basis::Instant, &pounds());
        assert_eq!(instant.basis, Basis::Net, "cards to drop stay after fees");
        assert_eq!(instant.value, Money::new(3 * 3 + 2 * 6, Currency::GBP));
    }

    #[test]
    fn stale_prices_count_and_the_oldest_is_told() {
        let mut book = PriceBook::default();
        book.sets.insert(10, set(10, &[5]));
        let mut fresh = set(20, &[8]);
        fresh.fetched_at = noon() + TimeDelta::hours(7);
        if let Price::Known(q) = &mut fresh.normal[0].price {
            q.fetched_at = noon() + TimeDelta::hours(7);
        }
        book.sets.insert(20, fresh);
        let cards = [
            HeldCard::named(10, "Card 0", false),
            HeldCard::named(20, "Card 0", false),
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
            basis: Basis::List,
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
