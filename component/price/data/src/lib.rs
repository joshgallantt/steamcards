//! The market domain's contract, satisfied by Steam and the disk: a set's
//! cards from the market's search and a card's order book, through the
//! Steam session's one market queue; the wallet from the CM connection; the
//! basis and Steam's pause in the config file, and prices in their own file
//! beside it. Imports `market` because the contract is declared there;
//! `market` imports nothing back.

use std::{
    sync::{Arc, Mutex},
    time::{Duration, UNIX_EPOCH},
};

use async_trait::async_trait;
use chrono::{DateTime, TimeDelta, Utc};
use config_file::{ConfigFile, PriceCache, StoredCard, StoredPause, StoredPrice, StoredSet};
use money::{Currency, Money};
use price::{
    Basis, Lookup, MarketPause, Offers, Price, PriceBook, PriceQuote, PriceRepository,
    PriceSettings, PricedCard, QuoteSource, SetPrices, Wallet,
};
use steam_api::{
    SteamClient,
    market::{self as steam, Listed, Market, OrderBook},
};

/// How long a set's prices are kept once looked up: shown dim with their
/// age after 6 hours, and gone after a week (research §1.3's cache).
const KEPT_FOR: TimeDelta = TimeDelta::days(7);

/// How long to wait for Steam to tell of the wallet once signed on, a tenth
/// of a second at a time: it comes as a session signs on.
const WALLET_WAITS: u32 = 10;
const WALLET_WAIT: Duration = Duration::from_millis(100);

/// The market as the Steam session sees it, and what's kept of it.
pub struct SteamPriceRepository {
    steam: Arc<SteamClient>,
    file: Arc<ConfigFile>,
    cache: Arc<PriceCache>,
    book: Mutex<Arc<PriceBook>>,
    wanted: Mutex<Vec<u32>>,
}

impl SteamPriceRepository {
    /// Takes up what was kept from before: the prices under a week old, and
    /// Steam's pause, if there was one.
    pub fn new(steam: Arc<SteamClient>, file: Arc<ConfigFile>, cache: Arc<PriceCache>) -> Self {
        if let Some(pause) = file.market().pause {
            steam.resume_market_pause(to_steam_pause(pause));
        }
        let now = Utc::now();
        let mut book = PriceBook::default();
        for set in cache.sets().into_iter().filter_map(to_set) {
            if is_kept(&set, now) {
                book.sets.insert(set.app_id, set);
            }
        }
        Self {
            steam,
            file,
            cache,
            book: Mutex::new(Arc::new(book)),
            wanted: Mutex::default(),
        }
    }

    /// Keeps Steam's pause in the config file when it has changed, so it
    /// outlasts a restart.
    fn keep_pause(&self) {
        let pause = self.steam.market_pause().map(to_stored_pause);
        if self.file.market().pause == pause {
            return;
        }
        if let Err(e) = self.file.save_market_pause(pause) {
            self.steam
                .log()
                .line(&format!("couldn't keep Steam's pause on the market: {e}"));
        }
    }

    /// The wallet's currency, which the market's prices are read in. Steam
    /// tells of the wallet as a session signs on: until it has, a session
    /// signs on, and it's waited for a moment. Why not, when it can't be
    /// told.
    async fn currency(&self) -> Result<Currency, String> {
        if let Some(wallet) = self.wallet() {
            return Ok(wallet.currency);
        }
        self.steam.connection().await.map_err(|e| e.to_string())?;
        for _ in 0..WALLET_WAITS {
            if let Some(wallet) = self.wallet() {
                return Ok(wallet.currency);
            }
            tokio::time::sleep(WALLET_WAIT).await;
        }
        Err("Steam hasn't said the wallet's currency yet".into())
    }
}

#[async_trait]
impl PriceRepository for SteamPriceRepository {
    fn book(&self) -> Arc<PriceBook> {
        Arc::clone(&self.book.lock().unwrap())
    }

    fn keep_set(&self, set: SetPrices) {
        let now = Utc::now();
        let book = {
            let mut book = self.book.lock().unwrap();
            let mut next = PriceBook::clone(&book);
            next.sets.insert(set.app_id, set);
            next.sets.retain(|_, set| is_kept(set, now));
            *book = Arc::new(next);
            Arc::clone(&book)
        };
        let stored = book.sets.values().map(to_stored_set).collect();
        if let Err(e) = self.cache.save_sets(stored) {
            // Only a restart would miss them: they'd be looked up again.
            self.steam
                .log()
                .line(&format!("couldn't keep prices on disk: {e}"));
        }
    }

    fn keep_offers(&self, market_hash_name: &str, offers: Offers) {
        let mut book = self.book.lock().unwrap();
        let mut next = PriceBook::clone(&book);
        next.offers.insert(market_hash_name.to_owned(), offers);
        *book = Arc::new(next);
    }

    async fn look_up_set(
        &self,
        app_id: u32,
        foil: bool,
    ) -> anyhow::Result<Lookup<Vec<PricedCard>>> {
        let currency = match self.currency().await {
            Ok(currency) => currency,
            Err(why) => return Ok(Lookup::Unanswered(why)),
        };
        let answer = self.steam.market_search(app_id, foil).await;
        self.keep_pause();
        let listed = match answer? {
            Market::Answer(listed) => listed,
            Market::Paused(pause) => return Ok(Lookup::Paused(to_pause(pause))),
            Market::Unanswered(why) => return Ok(Lookup::Unanswered(why)),
        };
        let now = Utc::now();
        Ok(Lookup::Found(
            listed
                .into_iter()
                .filter(|card| is_border(card, foil))
                .map(|card| to_priced_card(card, currency, now))
                .collect(),
        ))
    }

    async fn look_up_offers(&self, market_hash_name: &str) -> anyhow::Result<Lookup<Price>> {
        let answer = self.steam.order_book(market_hash_name).await;
        self.keep_pause();
        Ok(match answer? {
            Market::Answer(book) => Lookup::Found(to_offers(book, Utc::now())),
            Market::Paused(pause) => Lookup::Paused(to_pause(pause)),
            Market::Unanswered(why) => Lookup::Unanswered(why),
        })
    }

    fn wallet(&self) -> Option<Wallet> {
        let info = self.steam.wallet()?;
        // An account with no wallet is priced in dollars, as Valve's pages
        // ask the market when there's no wallet currency.
        let currency = u32::try_from(info.currency)
            .ok()
            .filter(|&id| info.has_wallet && id > 0)
            .map_or(Currency::USD, Currency::from_id);
        // The fees are Valve's defaults for now. The account's own are on a
        // signed-in inventory page (`g_rgWalletInfo`), which isn't read yet;
        // the defaults gave the right answer in every example the research
        // checked (§1.4).
        Some(Wallet::new(currency))
    }

    fn settings(&self) -> PriceSettings {
        let basis = match self.file.market().basis.as_str() {
            "net" => Basis::Net,
            "instant" => Basis::Instant,
            _ => Basis::List,
        };
        PriceSettings { basis }
    }

    fn save_settings(&self, settings: PriceSettings) -> anyhow::Result<()> {
        let basis = match settings.basis {
            Basis::List => "list",
            Basis::Net => "net",
            Basis::Instant => "instant",
        };
        self.file.save_market_basis(basis.to_owned())
    }

    fn wanted(&self) -> Vec<u32> {
        self.wanted.lock().unwrap().clone()
    }

    fn want(&self, app_ids: Vec<u32>) {
        *self.wanted.lock().unwrap() = app_ids;
    }
}

/// Whether a card the market listed is of the border asked for, as its
/// type says: "Portal 2 Foil Trading Card" or "Portal 2 Trading Card". A
/// type that says neither is taken to be.
fn is_border(card: &Listed, foil: bool) -> bool {
    if card.item_type.ends_with("Foil Trading Card") {
        foil
    } else if card.item_type.ends_with("Trading Card") {
        !foil
    } else {
        true
    }
}

/// A card the market listed, priced. The market's search doesn't say which
/// currency it answered in, so its price is written in the wallet's
/// currency, and failing that in dollars, and compared with the market's
/// own text (research §1.2, rule 4). Failing both, it's in whichever of
/// Valve's currencies its text is written as: shown so, and never counted
/// with the wallet's (rule 3), as an account without a wallet's prices
/// are. A price written as none isn't used.
fn to_priced_card(card: Listed, currency: Currency, now: DateTime<Utc>) -> PricedCard {
    let price = if card.sell_listings == 0 || card.sell_price <= 0 {
        Price::NoMarket
    } else {
        let written_in = [currency, Currency::USD]
            .into_iter()
            .chain(Currency::every())
            .find(|&c| is_written_as(Money::new(card.sell_price, c), &card.sell_price_text));
        match written_in {
            Some(c) => Price::Known(PriceQuote {
                ask: Some(Money::new(card.sell_price, c)),
                bid: None,
                ask_depth: Some(card.sell_listings),
                bid_depth: None,
                source: QuoteSource::Search,
                fetched_at: now,
            }),
            None => Price::failed(now),
        }
    };
    PricedCard {
        name: card.name,
        market_hash_name: card.hash_name,
        price,
    }
}

/// Whether the market's `text` is `money`: as Valve's pages write it, with
/// or without thousands separators, and dollars with or without " USD".
fn is_written_as(money: Money, text: &str) -> bool {
    let text = text.trim().replace('\u{a0}', " ");
    let text = match money.currency {
        Currency::USD => text.strip_suffix(" USD").unwrap_or(&text),
        _ => &text,
    };
    text == money.to_string() || text == money.grouped()
}

/// A card's order book, priced: its lowest listing and its best offer. As
/// SteamDB's extension reads it, a side counts only with orders on it.
fn to_offers(book: OrderBook, now: DateTime<Utc>) -> Price {
    let currency = Currency::from_id(book.currency);
    let ask = (book.sell_orders > 0 && book.lowest_ask > 0)
        .then(|| Money::new(book.lowest_ask, currency));
    let bid = (book.buy_orders > 0 && book.highest_bid > 0)
        .then(|| Money::new(book.highest_bid, currency));
    if ask.is_none() && bid.is_none() {
        return Price::NoMarket;
    }
    Price::Known(PriceQuote {
        ask,
        bid,
        ask_depth: Some(book.sell_orders),
        bid_depth: Some(book.buy_orders),
        source: QuoteSource::OrderBook,
        fetched_at: now,
    })
}

fn is_kept(set: &SetPrices, now: DateTime<Utc>) -> bool {
    now - set.fetched_at <= KEPT_FOR
}

fn to_pause(pause: steam::MarketPause) -> MarketPause {
    MarketPause {
        until: DateTime::<Utc>::from(pause.until),
        step: pause.step,
    }
}

fn to_stored_pause(pause: steam::MarketPause) -> StoredPause {
    StoredPause {
        until: DateTime::<Utc>::from(pause.until).timestamp(),
        step: pause.step.as_secs(),
    }
}

fn to_steam_pause(pause: StoredPause) -> steam::MarketPause {
    steam::MarketPause {
        until: UNIX_EPOCH + Duration::from_secs(u64::try_from(pause.until).unwrap_or_default()),
        step: Duration::from_secs(pause.step),
    }
}

fn to_stored_set(set: &SetPrices) -> StoredSet {
    let cards = |cards: &[PricedCard]| {
        cards
            .iter()
            .map(|card| StoredCard {
                name: card.name.clone(),
                market_hash_name: card.market_hash_name.clone(),
                price: to_stored_price(&card.price),
            })
            .collect()
    };
    StoredSet {
        app_id: set.app_id,
        fetched_at: set.fetched_at.timestamp(),
        retry_at: set.retry_at.map(|at| at.timestamp()),
        normal: cards(&set.normal),
        foil: cards(&set.foil),
    }
}

fn to_stored_price(price: &Price) -> StoredPrice {
    let state = |state: &str| StoredPrice {
        state: state.to_owned(),
        ..StoredPrice::default()
    };
    match price {
        Price::Pending => state("pending"),
        Price::NoMarket => state("no market"),
        Price::NotMarketable => state("not marketable"),
        Price::Failed { retry_at } => StoredPrice {
            retry_at: Some(retry_at.timestamp()),
            ..state("failed")
        },
        Price::Known(quote) => StoredPrice {
            ask: quote.ask.map(|m| m.minor),
            bid: quote.bid.map(|m| m.minor),
            ask_depth: quote.ask_depth,
            bid_depth: quote.bid_depth,
            currency: quote.ask.or(quote.bid).map_or(0, |m| m.currency.id()),
            source: match quote.source {
                QuoteSource::Search => "search",
                QuoteSource::OrderBook => "order book",
            }
            .to_owned(),
            fetched_at: quote.fetched_at.timestamp(),
            ..state("known")
        },
    }
}

/// A set as it was kept; `None` when any of it doesn't read, so it's looked
/// up afresh.
fn to_set(stored: StoredSet) -> Option<SetPrices> {
    let cards = |cards: Vec<StoredCard>| {
        cards
            .into_iter()
            .map(|card| {
                Some(PricedCard {
                    price: to_price(&card.price)?,
                    name: card.name,
                    market_hash_name: card.market_hash_name,
                })
            })
            .collect::<Option<Vec<_>>>()
    };
    Some(SetPrices {
        app_id: stored.app_id,
        normal: cards(stored.normal)?,
        foil: cards(stored.foil)?,
        fetched_at: time(stored.fetched_at)?,
        retry_at: match stored.retry_at {
            Some(at) => Some(time(at)?),
            None => None,
        },
    })
}

fn to_price(stored: &StoredPrice) -> Option<Price> {
    Some(match stored.state.as_str() {
        "pending" => Price::Pending,
        "no market" => Price::NoMarket,
        "not marketable" => Price::NotMarketable,
        "failed" => Price::Failed {
            retry_at: time(stored.retry_at?)?,
        },
        "known" => {
            let currency = Currency::from_id(stored.currency);
            Price::Known(PriceQuote {
                ask: stored.ask.map(|minor| Money::new(minor, currency)),
                bid: stored.bid.map(|minor| Money::new(minor, currency)),
                ask_depth: stored.ask_depth,
                bid_depth: stored.bid_depth,
                source: match stored.source.as_str() {
                    "order book" => QuoteSource::OrderBook,
                    _ => QuoteSource::Search,
                },
                fetched_at: time(stored.fetched_at)?,
            })
        }
        _ => return None,
    })
}

fn time(seconds: i64) -> Option<DateTime<Utc>> {
    DateTime::from_timestamp(seconds, 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(seconds: i64) -> DateTime<Utc> {
        time(seconds).unwrap()
    }

    fn listed(name: &str, price: i64, text: &str, listings: u32) -> Listed {
        Listed {
            hash_name: format!("620-{name}"),
            name: name.into(),
            sell_price: price,
            sell_price_text: text.into(),
            sell_listings: listings,
            item_type: "Portal 2 Trading Card".into(),
        }
    }

    fn ask(card: &PricedCard) -> Option<Money> {
        match &card.price {
            Price::Known(quote) => quote.ask,
            _ => None,
        }
    }

    #[test]
    fn a_listing_is_priced_in_the_currency_its_text_is_written_in() {
        let now = at(1_790_700_000);
        let pounds = Currency::GBP;
        let card = |text: &str| to_priced_card(listed("Chell", 6, text, 40), pounds, now);

        assert_eq!(ask(&card("£0.06")), Some(Money::new(6, Currency::GBP)));
        assert_eq!(
            ask(&card("$0.06")),
            Some(Money::new(6, Currency::USD)),
            "then dollars"
        );
        assert_eq!(ask(&card("$0.06 USD")), Some(Money::new(6, Currency::USD)));
        assert_eq!(
            ask(&card("0,06€")),
            Some(Money::new(6, Currency::EUR)),
            "neither: as it came, never counted with pounds"
        );
        assert_eq!(
            card("6 gold").price,
            Price::failed(now),
            "no currency at all: the price isn't used"
        );
        let euros = to_priced_card(listed("Chell", 100, "1,--€", 40), Currency::EUR, now);
        assert_eq!(ask(&euros), Some(Money::new(100, Currency::EUR)));
        let rupiah = Currency::from_id(10);
        let grouped = to_priced_card(listed("Chell", 123_400, "Rp 1 234", 40), rupiah, now);
        assert_eq!(ask(&grouped), Some(Money::new(123_400, rupiah)), "grouped");
        let spaced = to_priced_card(
            listed("Chell", 150, "1,50\u{a0}zł", 40),
            Currency::from_id(6),
            now,
        );
        assert!(ask(&spaced).is_some(), "a non-breaking space");
    }

    #[test]
    fn a_card_nobody_is_selling_has_no_market() {
        let now = at(1_790_700_000);
        let card = to_priced_card(listed("Atlas", 0, "", 0), Currency::GBP, now);
        assert_eq!(card.price, Price::NoMarket);
        assert_eq!(card.name, "Atlas");
        assert_eq!(card.market_hash_name, "620-Atlas");
    }

    #[test]
    fn a_listings_border_is_its_type() {
        let normal = listed("Chell", 6, "$0.06", 1);
        let foil = Listed {
            item_type: "Portal 2 Foil Trading Card".into(),
            ..normal.clone()
        };
        let other = Listed {
            item_type: "Carte à collectionner de Portal 2".into(),
            ..normal.clone()
        };
        assert!(is_border(&normal, false) && !is_border(&normal, true));
        assert!(is_border(&foil, true) && !is_border(&foil, false));
        assert!(
            is_border(&other, true) && is_border(&other, false),
            "taken as asked"
        );
    }

    #[test]
    fn an_order_book_is_a_listing_and_an_offer() {
        let now = at(1_790_700_000);
        let chell = OrderBook {
            lowest_ask: 5,
            highest_bid: 4,
            sell_orders: 4662,
            buy_orders: 41763,
            currency: 2,
        };
        let Price::Known(quote) = to_offers(chell, now) else {
            panic!("known");
        };
        assert_eq!(quote.ask, Some(Money::new(5, Currency::GBP)));
        assert_eq!(quote.bid, Some(Money::new(4, Currency::GBP)));
        assert_eq!(
            (quote.ask_depth, quote.bid_depth),
            (Some(4662), Some(41763))
        );
        assert_eq!(quote.source, QuoteSource::OrderBook);

        let only_offers = OrderBook {
            sell_orders: 0,
            lowest_ask: 0,
            ..chell
        };
        let Price::Known(quote) = to_offers(only_offers, now) else {
            panic!("known");
        };
        assert_eq!(quote.ask, None, "nobody selling, but someone buying");
        let nothing = OrderBook {
            buy_orders: 0,
            ..only_offers
        };
        assert_eq!(to_offers(nothing, now), Price::NoMarket);
    }

    #[test]
    fn every_price_is_kept_as_it_was() {
        let now = at(1_790_700_000);
        let prices = [
            Price::Pending,
            Price::NoMarket,
            Price::NotMarketable,
            Price::failed(now),
            to_offers(
                OrderBook {
                    lowest_ask: 11,
                    highest_bid: 8,
                    sell_orders: 2005,
                    buy_orders: 34378,
                    currency: 2,
                },
                now,
            ),
            to_priced_card(listed("Chell", 6, "£0.06", 40), Currency::GBP, now).price,
        ];
        for price in prices {
            assert_eq!(
                to_price(&to_stored_price(&price)),
                Some(price.clone()),
                "{price:?}"
            );
        }
        assert_eq!(
            to_price(&StoredPrice::default()),
            None,
            "a state it doesn't know"
        );
    }

    #[test]
    fn steams_pause_is_kept_to_the_second() {
        let pause = steam::MarketPause {
            until: UNIX_EPOCH + Duration::from_secs(1_790_712_345),
            step: Duration::from_secs(20 * 60),
        };
        let stored = to_stored_pause(pause);
        assert_eq!(
            stored,
            StoredPause {
                until: 1_790_712_345,
                step: 1_200,
            }
        );
        assert_eq!(to_steam_pause(stored), pause);
        assert_eq!(to_pause(pause).until, at(1_790_712_345));
    }
}
