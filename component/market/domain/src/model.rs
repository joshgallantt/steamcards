use std::{collections::BTreeMap, fmt, time::Duration};

use chrono::{DateTime, Utc};
use library::CardAsset;
use money::{Currency, Money};

use crate::rules::{FRESH_FOR, RETRY_FAILED, between, later};

/// The account's Steam wallet, as far as the market goes: its currency, and
/// the fees Steam takes from a sale. Amounts are in hundredths of its
/// currency, fees in basis points (500 is 5%).
///
/// The fee rules are Valve's own, from economy_common.js (research §1.4), in
/// whole numbers: they give what Valve's floating-point ones give at every
/// price up to 200,000 hundredths, with the defaults and with other
/// minimums, steps and fees.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Wallet {
    pub currency: Currency,
    /// The least a seller may get, and the least any fee is:
    /// `wallet_market_minimum`.
    pub market_minimum: i64,
    /// Prices go up in steps of this: `wallet_currency_increment`.
    pub increment: i64,
    /// Steam's fee, in basis points: `wallet_fee_percent`.
    pub steam_fee: u32,
    /// The game's publisher's fee, in basis points:
    /// `wallet_publisher_fee_percent_default`.
    pub publisher_fee: u32,
    /// The most a buyer may pay: `wallet_trade_max_balance`. `None` while it
    /// isn't known.
    pub trade_max: Option<i64>,
}

impl Wallet {
    /// A wallet in `currency`, with Valve's defaults: a minimum of 1, steps
    /// of 1, 5% for Steam and 10% for the publisher. They gave the right
    /// answer in every example the research checked.
    pub fn new(currency: Currency) -> Self {
        Self {
            currency,
            market_minimum: 1,
            increment: 1,
            steam_fee: 500,
            publisher_fee: 1_000,
            trade_max: None,
        }
    }

    /// What a buyer pays for a card listed to pay its seller `seller_gets`:
    /// Valve's `GetTotalWithFees`.
    pub fn buyer_pays(&self, seller_gets: i64) -> i64 {
        self.valid(seller_gets)
            + self.fee(seller_gets, self.publisher_fee)
            + self.fee(seller_gets, self.steam_fee)
    }

    /// What the seller gets when a buyer pays `buyer_pays`: Valve's
    /// `GetItemPriceFromTotal`. Some buyer prices can't occur: 22p pays 19p,
    /// and a card listed to pay 19p costs a buyer 21p. So a price to list
    /// at is `buyer_pays` of what the seller is to get.
    pub fn seller_gets(&self, buyer_pays: i64) -> i64 {
        let with_fees = 10_000 + i64::from(self.publisher_fee) + i64::from(self.steam_fee);
        let guess = buyer_pays.saturating_mul(10_000).div_euclid(with_fees);
        let mut base = self.valid(guess.min(buyer_pays - 2 * self.market_minimum));
        // At or under the answer now; it's at most a few steps up.
        for _ in 0..3 {
            let total = self.buyer_pays(base);
            if total == buyer_pays {
                return base;
            }
            if total < buyer_pays {
                base += self.increment;
            } else {
                base -= self.increment;
                break;
            }
        }
        base.max(self.market_minimum)
    }

    /// Valve's `ToValidMarketPrice`: at least the minimum, and a whole
    /// number of steps, rounded to the nearest.
    fn valid(&self, price: i64) -> i64 {
        if price <= self.market_minimum {
            return self.market_minimum;
        }
        if price <= self.increment {
            return self.increment;
        }
        if self.increment > 1 {
            let steps = (2 * price.abs() + self.increment) / (2 * self.increment);
            return price.signum() * steps * self.increment;
        }
        price
    }

    /// Valve's `CalculateFee`: a share of `base`, rounded down, then made a
    /// valid price. A share of nothing is no fee at all.
    fn fee(&self, base: i64, basis_points: u32) -> i64 {
        if basis_points == 0 {
            return 0;
        }
        self.valid(
            base.saturating_mul(i64::from(basis_points))
                .div_euclid(10_000),
        )
    }
}

/// What a card is worth, as the user chooses to see it (research §1.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Basis {
    /// What a buyer pays: its lowest listing.
    #[default]
    List,
    /// What listing it at its lowest listing pays you, after Steam's fees.
    Net,
    /// What selling it now pays you: its best offer, after Steam's fees.
    /// Only a card's order book has offers.
    Instant,
}

impl Basis {
    /// The basis cards still to drop are valued on. A card that hasn't
    /// dropped can't be sold to an offer now, and an order book a card would
    /// cost a request each, so on the instant basis they're valued after
    /// fees instead, and the figures say so.
    pub fn still_to_drop(self) -> Basis {
        match self {
            Basis::Instant => Basis::Net,
            basis => basis,
        }
    }
}

/// The market's settings, kept apart from the preferences so they never
/// hold a market type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct MarketSettings {
    /// How money is shown: list prices, unless the user chooses otherwise.
    pub basis: Basis,
}

/// Steam's pause on price lookups. When Steam turns one down, no lookup
/// goes until the pause ends; then one goes to see, and if that's turned
/// down too, the pause is longer. It outlasts a restart.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MarketPause {
    pub until: DateTime<Utc>,
    /// How long this pause is: 10 minutes at first, doubling each time
    /// Steam turns a lookup down again, to an hour at most.
    pub step: Duration,
}

/// Where a quote came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuoteSource {
    /// A game's set, priced a page of cards at a time: lowest listings only.
    Search,
    /// One card's order book: its lowest listing and its best offer.
    OrderBook,
}

/// What the market said a card sells for, and when. Both prices are what a
/// buyer pays, fees included.
#[derive(Debug, Clone, PartialEq)]
pub struct PriceQuote {
    /// The lowest listing.
    pub ask: Option<Money>,
    /// The best offer: what someone is waiting to pay for it.
    pub bid: Option<Money>,
    /// How many are listed.
    pub ask_depth: Option<u32>,
    /// How many are wanted.
    pub bid_depth: Option<u32>,
    pub source: QuoteSource,
    pub fetched_at: DateTime<Utc>,
}

impl PriceQuote {
    /// How old it is at `now`.
    pub fn age(&self, now: DateTime<Utc>) -> Duration {
        between(self.fetched_at, now)
    }

    /// Older than 6 hours: it's shown dim with its age, and still counts.
    pub fn is_stale(&self, now: DateTime<Utc>) -> bool {
        self.age(now) > FRESH_FOR
    }
}

/// What's known of a card's price (research §1.5). Only a known price ever
/// counts: one that isn't known is never taken as nothing.
#[derive(Debug, Clone, PartialEq)]
pub enum Price {
    /// Not looked up yet.
    Pending,
    Known(PriceQuote),
    /// It can be sold, but nobody is selling it.
    NoMarket,
    /// It can't be sold on the market at all.
    NotMarketable,
    /// Steam's answer couldn't be used. It's looked up again then.
    Failed {
        retry_at: DateTime<Utc>,
    },
}

impl Price {
    /// A lookup that failed at `at`: it's tried again a day later.
    pub fn failed(at: DateTime<Utc>) -> Price {
        Price::Failed {
            retry_at: later(at, RETRY_FAILED),
        }
    }
}

/// A card of a game's set as the market lists it, and its price.
#[derive(Debug, Clone, PartialEq)]
pub struct PricedCard {
    /// Its name as the game's set lists it: "Intro", where the market
    /// lists "Intro (Trading Card)".
    pub name: String,
    /// Its name on the market, exactly as Steam gives it.
    pub market_hash_name: String,
    pub price: Price,
}

/// A game's set of cards, priced: its normal cards and its foils, from one
/// lookup at one time.
#[derive(Debug, Clone, PartialEq)]
pub struct SetPrices {
    pub app_id: u32,
    pub normal: Vec<PricedCard>,
    pub foil: Vec<PricedCard>,
    /// When its prices were looked up; for a set whose only lookup failed,
    /// when that was.
    pub fetched_at: DateTime<Utc>,
    /// Its last lookup failed, and it's tried again then. The prices it
    /// has, if any, are from the lookup before.
    pub retry_at: Option<DateTime<Utc>>,
}

impl SetPrices {
    /// A set whose first lookup failed at `at`: nothing priced, and tried
    /// again a day later.
    pub fn failed(app_id: u32, at: DateTime<Utc>) -> Self {
        Self {
            app_id,
            normal: Vec::new(),
            foil: Vec::new(),
            fetched_at: at,
            retry_at: Some(later(at, RETRY_FAILED)),
        }
    }

    /// A card of the set by its name as the game's set lists it, normal or
    /// foil.
    pub fn card(&self, name: &str, foil: bool) -> Option<&PricedCard> {
        let border = if foil { &self.foil } else { &self.normal };
        border.iter().find(|c| c.name == name)
    }

    /// A card of the set by its market hash name.
    pub fn by_hash(&self, market_hash_name: &str) -> Option<&PricedCard> {
        self.normal
            .iter()
            .chain(&self.foil)
            .find(|c| c.market_hash_name == market_hash_name)
    }

    /// What a card of the set sells for, by its name.
    pub fn price(&self, name: &str, foil: bool) -> Price {
        self.card(name, foil)
            .map_or_else(|| self.unlisted(), |c| c.price.clone())
    }

    /// The price of a card of the set the market didn't list: failed, if
    /// the lookup did. Otherwise nobody is selling it. Whether the market
    /// lists a card nobody is selling at all is an open question (research
    /// §6, question 3); either way, it has no market.
    fn unlisted(&self) -> Price {
        match self.retry_at {
            Some(retry_at) => Price::Failed { retry_at },
            None => Price::NoMarket,
        }
    }

    /// When it's next due a lookup: when its prices turn 6 hours old, or
    /// after a failed lookup, a day after it.
    pub fn due_at(&self) -> DateTime<Utc> {
        self.retry_at
            .unwrap_or_else(|| later(self.fetched_at, FRESH_FOR))
    }
}

/// A card's order book as last looked up: what it said, and when. Nobody
/// buying or selling is an answer too, so an order book says when it was
/// looked up whatever it said.
#[derive(Debug, Clone, PartialEq)]
pub struct Offers {
    pub price: Price,
    pub looked_up_at: DateTime<Utc>,
}

/// Everything priced so far: each game's set, and each card's best offers
/// that have been looked up.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PriceBook {
    /// Sets, by app ID.
    pub sets: BTreeMap<u32, SetPrices>,
    /// Order books, by market hash name.
    pub offers: BTreeMap<String, Offers>,
}

impl PriceBook {
    /// A held card's price on `basis`. List and net come from its game's
    /// set, instant from its order book. A held card is worth the latest
    /// price for it, not the one it had when it dropped.
    pub fn price(&self, card: &HeldCard, basis: Basis) -> Price {
        if !card.marketable {
            return Price::NotMarketable;
        }
        let set = self.sets.get(&card.app_id);
        match basis {
            Basis::List | Basis::Net => {
                let Some(set) = set else {
                    return Price::Pending;
                };
                let listed = card
                    .market_hash_name
                    .as_deref()
                    .and_then(|hash| set.by_hash(hash))
                    .or_else(|| set.card(&card.name, card.foil));
                listed.map_or_else(|| set.unlisted(), |c| c.price.clone())
            }
            Basis::Instant => {
                // A card known only by its name has its hash name from its set.
                let hash = card.market_hash_name.as_deref().or_else(|| {
                    set?.card(&card.name, card.foil)
                        .map(|c| c.market_hash_name.as_str())
                });
                hash.and_then(|h| self.offers.get(h))
                    .map_or(Price::Pending, |offers| offers.price.clone())
            }
        }
    }
}

/// A card the account holds, as far as its value goes: a copy that dropped,
/// or one known only by its name and border.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeldCard {
    /// The game whose set it's from.
    pub app_id: u32,
    /// Its name as the game's set lists it.
    pub name: String,
    pub foil: bool,
    /// Its exact name on the market, when its copy was described.
    pub market_hash_name: Option<String>,
    pub marketable: bool,
}

impl HeldCard {
    /// A card known only by its name and whether it's a foil: from its
    /// game's card page, when its copy couldn't be described. Its price is
    /// its set's, by name.
    pub fn named(app_id: u32, name: &str, foil: bool) -> Self {
        Self {
            app_id,
            name: name.to_owned(),
            foil,
            market_hash_name: None,
            marketable: true,
        }
    }
}

impl From<&CardAsset> for HeldCard {
    fn from(asset: &CardAsset) -> Self {
        Self {
            app_id: asset.app_id,
            name: asset.name.clone(),
            foil: asset.foil,
            market_hash_name: Some(asset.market_hash_name.clone()),
            marketable: asset.marketable,
        }
    }
}

/// What cards held are worth: at least `total`, since cards that aren't
/// priced can only add to it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Held {
    pub total: Money,
    /// Cards counted in the total.
    pub priced: u32,
    /// Cards that can be sold but aren't counted: not priced yet, not
    /// known yet, nobody selling, a failed lookup, or priced in another
    /// currency.
    pub unpriced: u32,
    /// Cards that can't be sold, left out.
    pub not_marketable: u32,
    /// How old the oldest price over 6 hours old in the total is; `None`
    /// while every price counted is fresh.
    pub oldest: Option<Duration>,
}

/// A value that's an estimate: what cards still to drop are likely worth.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Estimate {
    pub value: Money,
    /// Foils are left out: how often one drops isn't known.
    pub excl_foils: bool,
    /// Games with drops left whose cards aren't priced, so aren't in it.
    pub unpriced_games: u32,
    /// The basis the cards still to drop are valued on: net when instant was
    /// asked for (see [`Basis::still_to_drop`]).
    pub basis: Basis,
}

/// What the market said to a lookup, that Steam has paused lookups, or
/// that the market couldn't be asked.
#[derive(Debug, Clone, PartialEq)]
pub enum Lookup<T> {
    Found(T),
    Paused(MarketPause),
    /// The market couldn't be asked, or didn't answer, for this reason: not
    /// signed in, no network, a server error twice, or the wallet's currency
    /// not known yet to read prices in. Nothing is wrong with the prices,
    /// so none is taken as failed: it's asked again soon.
    Unanswered(String),
}

/// Something the price watcher did, for the log.
#[derive(Debug, Clone, PartialEq)]
pub struct MarketEvent {
    pub kind: MarketEventKind,
    /// What happened, in the user's words.
    pub message: String,
}

/// What a market event is about, so the UI can show the ones that matter,
/// and write its own line from what each carries: a game's name for its app
/// ID, a time on the clock for a time. The event's message says the same,
/// in the market's own words, with durations.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MarketEventKind {
    /// Every game wanted priced has been looked up, for now: how many, and
    /// when the next round is due.
    AllPriced {
        games: usize,
        next_round: Option<DateTime<Utc>>,
    },
    /// Steam turned a lookup down: lookups wait until the pause ends.
    Paused(MarketPause),
    /// Steam's pause is over: the first lookup since wasn't turned down.
    Resumed,
    /// A game's prices couldn't be looked up, by app ID: the market's
    /// answer couldn't be used. They're tried again a day later.
    Failed(u32),
    /// The market couldn't be asked, or didn't answer: it's asked again
    /// then, and nothing is taken as failed meanwhile.
    Unanswered { retry_at: DateTime<Utc> },
}

/// Why something asked of the market didn't happen, in the user's terms.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MarketError {
    /// A change didn't stick: the settings couldn't be saved.
    Unavailable,
    /// Steam has paused price lookups until the pause ends.
    Paused(MarketPause),
    /// The market couldn't be asked just now: no sign-in, or no network.
    Unanswered,
}

impl fmt::Display for MarketError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MarketError::Unavailable => write!(f, "the market settings couldn't be saved"),
            MarketError::Paused(_) => write!(f, "Steam has paused price lookups"),
            MarketError::Unanswered => write!(f, "the market couldn't be asked just now"),
        }
    }
}

impl std::error::Error for MarketError {}

#[cfg(test)]
mod tests {
    use chrono::TimeDelta;

    use super::*;

    const HOUR: Duration = Duration::from_secs(60 * 60);

    fn pounds() -> Wallet {
        Wallet::new(Currency::GBP)
    }

    fn noon() -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2026-09-29T12:00:00Z")
            .unwrap()
            .to_utc()
    }

    fn listed(name: &str, hash: &str, ask: i64) -> PricedCard {
        PricedCard {
            name: name.into(),
            market_hash_name: hash.into(),
            price: Price::Known(PriceQuote {
                ask: Some(Money::new(ask, Currency::GBP)),
                bid: None,
                ask_depth: Some(10),
                bid_depth: None,
                source: QuoteSource::Search,
                fetched_at: noon(),
            }),
        }
    }

    /// What undercutting lists at, and pays: a step under the lowest
    /// listing, never under the best offer (ui.md §7).
    fn undercut(wallet: &Wallet, ask: i64, bid: i64) -> (i64, i64) {
        let gets = (wallet.seller_gets(ask) - wallet.increment).max(wallet.seller_gets(bid));
        (wallet.buyer_pays(gets), gets)
    }

    #[test]
    fn a_seller_gets_what_a_buyer_pays_less_steams_fees() {
        // research §1.4, and ui.md §7's 62p.
        let table = [
            (3, 1),
            (4, 2),
            (5, 3),
            (6, 4),
            (8, 6),
            (11, 9),
            (13, 11),
            (23, 20),
            (29, 26),
            (62, 55),
            (63, 56),
            (100, 88),
        ];
        let wallet = pounds();
        for (buyer, seller) in table {
            assert_eq!(wallet.seller_gets(buyer), seller, "a buyer paying {buyer}p");
            assert_eq!(
                wallet.buyer_pays(seller),
                buyer,
                "a seller getting {seller}p"
            );
        }
    }

    #[test]
    fn some_buyer_prices_cant_occur() {
        let wallet = pounds();
        assert_eq!(wallet.seller_gets(22), 19);
        assert_eq!(wallet.buyer_pays(19), 21, "19p lists at 21p, not 22p");
        let never: Vec<i64> = (3..200)
            .filter(|&t| wallet.buyer_pays(wallet.seller_gets(t)) != t)
            .collect();
        assert_eq!(never.len(), 23, "{never:?}");
        assert!(
            (1..5_000).all(|b| wallet.seller_gets(wallet.buyer_pays(b)) == b),
            "every seller amount lists at a price that pays it"
        );
    }

    #[test]
    fn undercutting_lists_a_step_under_the_lowest_listing_never_under_the_best_offer() {
        // ui.md §7: Thanatos, Nyx, Zagreus, Madison and The Boy.
        let wallet = pounds();
        assert_eq!(undercut(&wallet, 62, 41), (61, 54));
        assert_eq!(undercut(&wallet, 9, 7), (8, 6));
        assert_eq!(undercut(&wallet, 8, 6), (7, 5));
        assert_eq!(undercut(&wallet, 5, 4), (4, 2));
        assert_eq!(undercut(&wallet, 4, 3).0, 3, "The Boy lists at 3p");
    }

    #[test]
    fn prices_keep_to_the_wallets_minimum_and_steps() {
        // Worked out with Valve's economy_common.js as it stands.
        let wallet = Wallet {
            market_minimum: 5,
            increment: 5,
            ..pounds()
        };
        assert_eq!(wallet.valid(3), 5, "at least the minimum");
        assert_eq!(wallet.valid(12), 10, "to the nearest step");
        assert_eq!(wallet.valid(13), 15, "halfway rounds up");
        assert_eq!(wallet.buyer_pays(100), 100 + 10 + 5);
        assert_eq!(
            wallet.buyer_pays(10),
            10 + 5 + 5,
            "each fee at least the minimum"
        );
        assert_eq!(wallet.seller_gets(115), 100);
        assert_eq!(wallet.seller_gets(20), 10);
        let free = Wallet {
            steam_fee: 0,
            ..pounds()
        };
        assert_eq!(free.buyer_pays(100), 110, "no Steam fee at all");
    }

    #[test]
    fn instant_values_cards_still_to_drop_after_fees() {
        assert_eq!(Basis::default(), Basis::List);
        assert_eq!(Basis::Instant.still_to_drop(), Basis::Net);
        assert_eq!(Basis::Net.still_to_drop(), Basis::Net);
        assert_eq!(Basis::List.still_to_drop(), Basis::List);
    }

    #[test]
    fn a_quote_over_six_hours_old_is_stale() {
        let quote = match listed("Madison", "960910-Madison", 5).price {
            Price::Known(q) => q,
            other => panic!("{other:?}"),
        };
        let at = |hours: i64| noon() + TimeDelta::hours(hours);
        assert!(!quote.is_stale(at(6)));
        assert!(quote.is_stale(at(8)));
        assert_eq!(quote.age(at(8)), 8 * HOUR);
        assert_eq!(quote.age(at(-1)), Duration::ZERO, "a clock put back");
        assert_eq!(
            Price::failed(noon()),
            Price::Failed { retry_at: at(24) },
            "tried again a day later"
        );
    }

    #[test]
    fn a_sets_cards_are_found_by_name_and_by_hash_name() {
        let portal = SetPrices {
            app_id: 620,
            normal: vec![
                listed("Intro", "620-Intro (Trading Card)", 8),
                listed("Chell", "620-Chell", 6),
            ],
            foil: vec![listed("Intro", "620-Intro (Foil Trading Card)", 63)],
            fetched_at: noon(),
            retry_at: None,
        };
        assert_eq!(
            portal.card("Intro", true).unwrap().market_hash_name,
            "620-Intro (Foil Trading Card)"
        );
        assert_eq!(portal.by_hash("620-Chell").unwrap().name, "Chell");
        assert_eq!(portal.card("Chell", true), None);
        assert_eq!(portal.price("Atlas", false), Price::NoMarket, "not listed");
        assert_eq!(portal.due_at(), noon() + TimeDelta::hours(6));

        let failed = SetPrices {
            retry_at: Some(noon() + TimeDelta::hours(30)),
            ..portal
        };
        assert!(
            matches!(failed.price("Chell", false), Price::Known(_)),
            "the prices from before"
        );
        assert_eq!(
            failed.price("Atlas", false),
            Price::Failed {
                retry_at: noon() + TimeDelta::hours(30)
            }
        );
        assert_eq!(failed.due_at(), noon() + TimeDelta::hours(30));
        assert_eq!(
            SetPrices::failed(620, noon()).due_at(),
            noon() + TimeDelta::hours(24)
        );
    }

    #[test]
    fn a_held_cards_price_depends_on_the_basis() {
        let mut book = PriceBook::default();
        book.sets.insert(
            960_910,
            SetPrices {
                app_id: 960_910,
                normal: vec![listed("Madison", "960910-Madison", 5)],
                foil: Vec::new(),
                fetched_at: noon(),
                retry_at: None,
            },
        );
        let madison = HeldCard {
            market_hash_name: Some("960910-Madison".into()),
            ..HeldCard::named(960_910, "Madison", false)
        };
        let by_name = HeldCard::named(960_910, "Madison", false);
        assert!(matches!(book.price(&madison, Basis::List), Price::Known(_)));
        assert_eq!(
            book.price(&madison, Basis::Net),
            book.price(&by_name, Basis::Net)
        );
        assert_eq!(
            book.price(&madison, Basis::Instant),
            Price::Pending,
            "no order book yet"
        );
        book.offers.insert(
            "960910-Madison".into(),
            Offers {
                price: Price::NoMarket,
                looked_up_at: noon(),
            },
        );
        assert_eq!(
            book.price(&by_name, Basis::Instant),
            Price::NoMarket,
            "its hash from its set"
        );

        let scott = HeldCard::named(960_910, "Scott", false);
        assert_eq!(book.price(&scott, Basis::List), Price::NoMarket);
        let hades = HeldCard::named(1_145_360, "Zagreus", false);
        assert_eq!(book.price(&hades, Basis::List), Price::Pending);
        let unsellable = HeldCard {
            marketable: false,
            ..madison
        };
        assert_eq!(book.price(&unsellable, Basis::List), Price::NotMarketable);
    }

    #[test]
    fn a_dropped_copy_is_held_as_the_card_it_is() {
        let asset = CardAsset {
            asset_id: 31_002,
            app_id: 960_910,
            name: "Madison".into(),
            market_hash_name: "960910-Madison".into(),
            foil: false,
            marketable: true,
            tradable: true,
        };
        assert_eq!(
            HeldCard::from(&asset),
            HeldCard {
                app_id: 960_910,
                name: "Madison".into(),
                foil: false,
                market_hash_name: Some("960910-Madison".into()),
                marketable: true,
            }
        );
    }
}
