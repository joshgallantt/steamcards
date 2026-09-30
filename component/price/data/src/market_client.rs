use std::{sync::Arc, time::Duration};

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use game::AppId;
use money::{Currency, Money};
use price::{Lookup, MarketPause, Price, PriceQuote, PricedCard, QuoteSource, Wallet};
use steam_api::SteamClient;

use crate::market::{
    Listed, MAX_SET_PAGES, MarketPace, MarketQueue, OrderBook, QUERY_ACTION, orderbook_path,
    read_order_book, read_search, search_path,
};

/// How long to wait for Steam to tell of the wallet once signed on, a tenth
/// of a second at a time: it comes as a session signs on.
const WALLET_WAITS: u32 = 10;
const WALLET_WAIT: Duration = Duration::from_millis(100);

/// The market's say on what cards are worth, and the wallet's currency.
#[async_trait]
pub trait MarketClient: Send + Sync {
    /// A game's cards as the market lists them now, normal cards or foils,
    /// with their lowest listings, in the wallet's currency.
    async fn look_up_set(
        &self,
        app_id: AppId,
        foil: bool,
    ) -> anyhow::Result<Lookup<Vec<PricedCard>>>;

    /// A card's order book, by its market hash name: its lowest listing and
    /// its best offer.
    async fn look_up_offers(&self, market_hash_name: &str) -> anyhow::Result<Lookup<Price>>;

    /// The account's wallet, once Steam has said.
    fn wallet(&self) -> Option<Wallet>;

    /// Steam's pause on market requests, if there is one.
    fn pause(&self) -> Option<MarketPause>;

    /// Takes up Steam's pause on market requests from before a restart.
    fn resume(&self, pause: MarketPause);
}

/// The market as the Steam session sees it, signed in: every request through
/// one queue, at the market's pace.
pub struct SteamMarketClient {
    steam: Arc<SteamClient>,
    /// Every request to the market goes through here, one at a time.
    queue: MarketQueue,
}

impl SteamMarketClient {
    pub fn new(steam: Arc<SteamClient>) -> Self {
        Self::with_pace(steam, MarketPace::default())
    }

    /// The same, with the market's requests at another pace: for tests that
    /// can't wait minutes.
    pub fn with_pace(steam: Arc<SteamClient>, pace: MarketPace) -> Self {
        Self {
            steam,
            queue: MarketQueue::new(pace),
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

    /// A game's cards as the market lists them now, normal cards or foils,
    /// with their lowest listings: every page of `search/render`, signed in,
    /// each through the market's queue. A card is listed once, should a
    /// page repeat one. Without a sign-in to ask as, the market goes
    /// unasked.
    async fn search(&self, app_id: u32, foil: bool) -> anyhow::Result<Lookup<Vec<Listed>>> {
        let who = match self.steam.web_login(false).await {
            Ok(who) => who,
            Err(e) => return Ok(Lookup::Unanswered(e.to_string())),
        };
        let mut listed: Vec<Listed> = Vec::new();
        let mut start = 0;
        for _ in 0..MAX_SET_PAGES {
            let path = search_path(app_id, foil, start);
            let reply = match self
                .queue
                .send(true, || self.steam.get_once(&path, Some(&who), &[]))
                .await?
            {
                Lookup::Found(reply) => reply,
                Lookup::Paused(pause) => return Ok(Lookup::Paused(pause)),
                Lookup::Unanswered(why) => return Ok(Lookup::Unanswered(why)),
            };
            let page = read_search(&reply.body)?;
            let read = u32::try_from(page.listed.len()).unwrap_or(u32::MAX);
            for card in page.listed {
                if !listed.iter().any(|l| l.hash_name == card.hash_name) {
                    listed.push(card);
                }
            }
            start += if page.page_size > 0 {
                page.page_size
            } else {
                read
            };
            if read == 0 || start >= page.total {
                break;
            }
        }
        Ok(Lookup::Found(listed))
    }

    /// A card's order book, by its market hash name: asked for signed in,
    /// since only then is it expected to answer in the wallet's currency,
    /// and signed out when the answer to that is a web page, as it is for
    /// some sessions (research §1.1, D5).
    async fn order_book(&self, market_hash_name: &str) -> anyhow::Result<Lookup<OrderBook>> {
        let path = orderbook_path(market_hash_name);
        let who = match self.steam.web_login(false).await {
            Ok(who) => who,
            Err(e) => return Ok(Lookup::Unanswered(e.to_string())),
        };
        let signed_in = self
            .queue
            .send(true, || {
                self.steam.get_once(&path, Some(&who), &[QUERY_ACTION])
            })
            .await?;
        let reply = match signed_in {
            Lookup::Paused(pause) => return Ok(Lookup::Paused(pause)),
            Lookup::Unanswered(why) => return Ok(Lookup::Unanswered(why)),
            Lookup::Found(reply) if !reply.html => reply,
            Lookup::Found(_) => {
                self.steam
                    .log()
                    .line("the order book came as a web page signed in: asking signed out");
                match self
                    .queue
                    .send(false, || self.steam.get_once(&path, None, &[QUERY_ACTION]))
                    .await?
                {
                    Lookup::Found(reply) => reply,
                    Lookup::Paused(pause) => return Ok(Lookup::Paused(pause)),
                    Lookup::Unanswered(why) => return Ok(Lookup::Unanswered(why)),
                }
            }
        };
        read_order_book(&reply.body).map(Lookup::Found)
    }
}

#[async_trait]
impl MarketClient for SteamMarketClient {
    async fn look_up_set(
        &self,
        app_id: AppId,
        foil: bool,
    ) -> anyhow::Result<Lookup<Vec<PricedCard>>> {
        let currency = match self.currency().await {
            Ok(currency) => currency,
            Err(why) => return Ok(Lookup::Unanswered(why)),
        };
        let listed = match self.search(app_id.0, foil).await? {
            Lookup::Found(listed) => listed,
            Lookup::Paused(pause) => return Ok(Lookup::Paused(pause)),
            Lookup::Unanswered(why) => return Ok(Lookup::Unanswered(why)),
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
        Ok(match self.order_book(market_hash_name).await? {
            Lookup::Found(book) => Lookup::Found(to_offers(book, Utc::now())),
            Lookup::Paused(pause) => Lookup::Paused(pause),
            Lookup::Unanswered(why) => Lookup::Unanswered(why),
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

    fn pause(&self) -> Option<MarketPause> {
        self.queue.pause()
    }

    fn resume(&self, pause: MarketPause) {
        self.queue.resume(pause);
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

#[cfg(test)]
mod tests {
    use super::*;

    fn at(seconds: i64) -> DateTime<Utc> {
        DateTime::from_timestamp(seconds, 0).unwrap()
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
}
