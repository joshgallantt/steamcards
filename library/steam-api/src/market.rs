//! The Steam market on steamcommunity.com: a game's cards with their lowest
//! listings (`search/render`), and one card's order book (`orderbook`), read
//! as Steam's own pages, SteamDB's extension and Steam Economy Enhancer read
//! them (research §1.1).
//!
//! The market limits requests harder than the rest of the site, and meets
//! quick retries by blocking for longer. So every market request goes
//! through one queue, one at a time, spaced out (research §1.3): signed in,
//! 5 seconds apart and up to a second more; signed out, 12 seconds. A 429
//! pauses every market request for 10 minutes, and nothing is asked
//! meanwhile; then one request goes to see, and if that's turned down too,
//! the pause doubles, to an hour at most. A server error is asked once more,
//! 30 seconds later. The queue sits on top of the site's own gap between
//! requests, never in place of it, and the site's quick retries never apply.
//!
//! A market that couldn't be asked, or didn't answer (no sign-in, no
//! network, a server error twice), is told apart from an answer that can't
//! be used: nothing is wrong with what was asked for, and it's worth asking
//! again soon.

use std::{
    future::Future,
    sync::Mutex,
    time::{Duration, SystemTime},
};

use anyhow::{anyhow, bail};
use reqwest::StatusCode;
use serde::Deserialize;
use tokio::time::Instant;

use crate::{
    community::Reply,
    inventory::{STEAM_APP, card_name},
};

/// How far apart market requests go, and how long Steam's pause lasts. The
/// default is the research's; tests may go faster.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MarketPace {
    /// After a request, before a signed-in one: 5 seconds.
    pub signed_in: Duration,
    /// Up to this much more, at random, before a signed-in request: a second.
    pub jitter: Duration,
    /// After a request, before a signed-out one: 12 seconds, since signed
    /// out, the market allows about 25 of a kind in 5 minutes.
    pub signed_out: Duration,
    /// The pause after Steam turns a request down: 10 minutes.
    pub first_pause: Duration,
    /// The longest pause, however often Steam turns the one sent to see
    /// down: an hour.
    pub longest_pause: Duration,
    /// Before asking again after a server error: 30 seconds.
    pub after_server_error: Duration,
}

impl Default for MarketPace {
    fn default() -> Self {
        Self {
            signed_in: Duration::from_secs(5),
            jitter: Duration::from_secs(1),
            signed_out: Duration::from_secs(12),
            first_pause: Duration::from_secs(10 * 60),
            longest_pause: Duration::from_secs(60 * 60),
            after_server_error: Duration::from_secs(30),
        }
    }
}

/// Steam's pause on market requests, after it turned one down.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MarketPause {
    /// When it ends. Once it has, the next request goes to see.
    pub until: SystemTime,
    /// How long it is: if the request sent to see is turned down too, the
    /// next is twice as long.
    pub step: Duration,
}

/// What a market request came to: the market's answer, Steam's pause, or
/// no answer at all.
#[derive(Debug, Clone, PartialEq)]
pub enum Market<T> {
    Answer(T),
    Paused(MarketPause),
    /// The market couldn't be asked, or didn't answer, for this reason: not
    /// signed in, no network, or a server error twice.
    Unanswered(String),
}

/// A card of a game's set as the market lists it: one result of
/// `search/render`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Listed {
    /// Its name on the market, exactly as Steam gives it:
    /// "620-Intro (Trading Card)".
    pub hash_name: String,
    /// Its name as its game's set lists it: the market's name for it
    /// ("Intro (Trading Card)") without Steam's suffix.
    pub name: String,
    /// Its lowest listing, what a buyer pays, in hundredths: `sell_price`.
    pub sell_price: i64,
    /// The same, as the market writes it: the only sign of its currency.
    pub sell_price_text: String,
    /// How many are listed.
    pub sell_listings: u32,
    /// What it is: "Portal 2 Trading Card", or "Portal 2 Foil Trading
    /// Card".
    pub item_type: String,
}

/// A card's order book. Every amount is in hundredths of `currency`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OrderBook {
    /// `amtMinSellOrder`: the lowest listing.
    pub lowest_ask: i64,
    /// `amtMaxBuyOrder`: the best offer.
    pub highest_bid: i64,
    /// `cSellOrders`: how many are listed.
    pub sell_orders: u32,
    /// `cBuyOrders`: how many are wanted.
    pub buy_orders: u32,
    /// `eCurrency`, read every time. Signed out, the order book answers in
    /// the currency of the country asked from; signed in, it's expected to
    /// answer in the wallet's.
    pub currency: u32,
}

/// The most pages of a set read: 10 cards a page, and a set has 15 at most.
pub(crate) const MAX_SET_PAGES: u32 = 10;

/// The market's requests, one at a time, at the market's pace.
pub(crate) struct MarketQueue {
    pace: MarketPace,
    /// Held for the whole of a request, waits and all: one at a time. When
    /// the last one finished.
    turn: tokio::sync::Mutex<Option<Instant>>,
    /// Steam's pause, when there is one: its end on tokio's clock, and on
    /// the system's as it was when it began. Past its end, it lasts until a
    /// request gets through.
    pause: Mutex<Option<(Instant, MarketPause)>>,
}

impl MarketQueue {
    pub(crate) fn new(pace: MarketPace) -> Self {
        Self {
            pace,
            turn: tokio::sync::Mutex::new(None),
            pause: Mutex::new(None),
        }
    }

    /// Sends one market request with `send` when its turn comes, signed in
    /// or out. While Steam's pause lasts, nothing is sent. A 429 pauses the
    /// market; a server error is asked once more, a while later; anything
    /// else ends the pause. No answer, or a server error twice, is
    /// [`Market::Unanswered`]; errs when the market said no.
    pub(crate) async fn send<F, Fut>(
        &self,
        signed_in: bool,
        send: F,
    ) -> anyhow::Result<Market<Reply>>
    where
        F: Fn() -> Fut,
        Fut: Future<Output = anyhow::Result<Reply>>,
    {
        let mut last = self.turn.lock().await;
        if let Some(pause) = self.paused() {
            return Ok(Market::Paused(pause));
        }
        let mut asked_again = false;
        loop {
            if let Some(at) = *last {
                tokio::time::sleep_until(at + self.gap(signed_in)).await;
            }
            let answer = send().await;
            *last = Some(Instant::now());
            let reply = match answer {
                Ok(reply) => reply,
                Err(e) => return Ok(Market::Unanswered(e.to_string())),
            };
            if reply.status == StatusCode::TOO_MANY_REQUESTS {
                return Ok(Market::Paused(self.turned_down()));
            }
            *self.pause.lock().unwrap() = None;
            if reply.status.is_server_error() && !asked_again {
                asked_again = true;
                tokio::time::sleep(self.pace.after_server_error).await;
                continue;
            }
            if reply.status.is_server_error() {
                return Ok(Market::Unanswered(format!(
                    "steamcommunity.com's market said {}, twice",
                    reply.status
                )));
            }
            if !reply.status.is_success() {
                bail!("steamcommunity.com's market said {}", reply.status);
            }
            return Ok(Market::Answer(reply));
        }
    }

    /// Steam's pause: while it lasts, and once over, until a request gets
    /// through.
    pub(crate) fn pause(&self) -> Option<MarketPause> {
        self.pause.lock().unwrap().map(|(_, pause)| pause)
    }

    /// Takes up a pause from before a restart.
    pub(crate) fn resume(&self, pause: MarketPause) {
        let left = pause
            .until
            .duration_since(SystemTime::now())
            .unwrap_or_default();
        *self.pause.lock().unwrap() = Some((Instant::now() + left, pause));
    }

    /// The pause, while it lasts: until either clock says it's over. Tokio's
    /// doesn't count the time a computer sleeps, and Steam's pause does.
    fn paused(&self) -> Option<MarketPause> {
        let (until, pause) = (*self.pause.lock().unwrap())?;
        (Instant::now() < until && SystemTime::now() < pause.until).then_some(pause)
    }

    /// Steam turned a request down: a pause, twice as long as the last if
    /// this was the request sent to see.
    fn turned_down(&self) -> MarketPause {
        let mut paused = self.pause.lock().unwrap();
        let step = paused.map_or(self.pace.first_pause, |(_, last)| {
            (last.step * 2).min(self.pace.longest_pause)
        });
        let pause = MarketPause {
            until: SystemTime::now() + step,
            step,
        };
        *paused = Some((Instant::now() + step, pause));
        pause
    }

    fn gap(&self, signed_in: bool) -> Duration {
        if signed_in {
            self.pace.signed_in + rand::random_range(Duration::ZERO..=self.pace.jitter)
        } else {
            self.pace.signed_out
        }
    }
}

/// `search/render` for a page of a game's cards: normal ones, or foils, 10
/// to a page from `start`, by name. Exactly as research §1.1 has it.
pub(crate) fn search_path(app_id: u32, foil: bool, start: u32) -> String {
    format!(
        "/market/search/render/?norender=1&appid={STEAM_APP}\
         &category_{STEAM_APP}_Game%5B%5D=tag_app_{app_id}\
         &category_{STEAM_APP}_item_class%5B%5D=tag_item_class_2\
         &category_{STEAM_APP}_cardborder%5B%5D=tag_cardborder_{}\
         &sort_column=name&sort_dir=asc&start={start}",
        u8::from(foil)
    )
}

/// One page of a game's cards.
#[derive(Debug)]
pub(crate) struct SearchPage {
    pub(crate) listed: Vec<Listed>,
    /// How many cards there are in all.
    pub(crate) total: u32,
    /// How many a page holds: 10, whatever's asked.
    pub(crate) page_size: u32,
}

#[derive(Deserialize)]
struct SearchReply {
    success: bool,
    #[serde(default)]
    pagesize: u32,
    #[serde(default)]
    total_count: u32,
    #[serde(default)]
    results: Vec<SearchResult>,
}

// `sale_price_text` is left unread: it's undocumented, and not what a
// seller gets (research §1.1).
#[derive(Deserialize)]
struct SearchResult {
    hash_name: String,
    name: String,
    #[serde(default)]
    sell_price: i64,
    #[serde(default)]
    sell_price_text: String,
    #[serde(default)]
    sell_listings: u32,
    #[serde(default)]
    asset_description: AssetDescription,
}

#[derive(Deserialize, Default)]
struct AssetDescription {
    #[serde(default, rename = "type")]
    item_type: String,
}

/// Reads a page of `search/render`.
pub(crate) fn read_search(body: &str) -> anyhow::Result<SearchPage> {
    let reply: SearchReply = serde_json::from_str(body)
        .map_err(|e| anyhow!("the market's list of cards didn't read ({e})"))?;
    if !reply.success {
        bail!("the market didn't list the cards");
    }
    Ok(SearchPage {
        total: reply.total_count,
        page_size: reply.pagesize,
        listed: reply
            .results
            .into_iter()
            .map(|r| Listed {
                name: card_name(&r.name),
                hash_name: r.hash_name,
                sell_price: r.sell_price,
                sell_price_text: r.sell_price_text,
                sell_listings: r.sell_listings,
                item_type: r.asset_description.item_type,
            })
            .collect(),
    })
}

/// `orderbook` for one card, as Steam's own new market asks:
/// `q=Load&qp=[753,"<hash name>"]`, the query encoded as a form's is.
pub(crate) fn orderbook_path(market_hash_name: &str) -> String {
    let qp = serde_json::json!([STEAM_APP, market_hash_name]).to_string();
    let query: String = reqwest::Url::parse_with_params(
        "https://steamcommunity.com/market/orderbook",
        [("q", "Load"), ("qp", qp.as_str())],
    )
    .ok()
    .and_then(|url| url.query().map(str::to_owned))
    .unwrap_or_default();
    format!("/market/orderbook?{query}")
}

/// The header Steam's own pages send with `orderbook`, as SteamDB's
/// extension does too.
pub(crate) const QUERY_ACTION: (&str, &str) = ("X-Valve-Request-Type", "queryAction");

/// An order book's reply comes wrapped in an outer `data` since about
/// 2026-08-14 (SEE PR #331), and bare before.
#[derive(Deserialize)]
#[serde(untagged)]
enum OrderBookReply {
    Wrapped { data: OrderBookAnswer },
    Bare(OrderBookAnswer),
}

#[derive(Deserialize)]
struct OrderBookAnswer {
    success: bool,
    data: Option<OrderBookData>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct OrderBookData {
    #[serde(default)]
    amt_min_sell_order: i64,
    #[serde(default)]
    amt_max_buy_order: i64,
    #[serde(default)]
    c_sell_orders: u32,
    #[serde(default)]
    c_buy_orders: u32,
    e_currency: Option<u32>,
}

/// Reads `orderbook`'s reply, in either shape.
pub(crate) fn read_order_book(body: &str) -> anyhow::Result<OrderBook> {
    let reply: OrderBookReply = serde_json::from_str(body)
        .map_err(|e| anyhow!("the market's order book didn't read ({e})"))?;
    let (OrderBookReply::Wrapped { data: answer } | OrderBookReply::Bare(answer)) = reply;
    // An unknown or localised hash name is `success: false`.
    let data = answer
        .data
        .filter(|_| answer.success)
        .ok_or_else(|| anyhow!("the market has no order book for that card"))?;
    Ok(OrderBook {
        lowest_ask: data.amt_min_sell_order,
        highest_bid: data.amt_max_buy_order,
        sell_orders: data.c_sell_orders,
        buy_orders: data.c_buy_orders,
        currency: data
            .e_currency
            .ok_or_else(|| anyhow!("the market's order book didn't say its currency"))?,
    })
}

#[cfg(test)]
mod tests {
    use std::{
        collections::VecDeque,
        sync::{Arc, Mutex},
    };

    use super::*;

    const SECOND: Duration = Duration::from_secs(1);
    const MINUTE: Duration = Duration::from_secs(60);

    /// A market that answers with these statuses in turn, and notes when
    /// each request went.
    #[derive(Clone, Default)]
    struct Stand {
        answers: Arc<Mutex<VecDeque<u16>>>,
        sent: Arc<Mutex<Vec<Instant>>>,
    }

    impl Stand {
        fn answering(statuses: &[u16]) -> Self {
            let stand = Self::default();
            stand.answers.lock().unwrap().extend(statuses);
            stand
        }

        async fn reply(&self) -> anyhow::Result<Reply> {
            self.sent.lock().unwrap().push(Instant::now());
            let status = self.answers.lock().unwrap().pop_front().unwrap_or(200);
            Ok(Reply {
                status: StatusCode::from_u16(status).unwrap(),
                html: false,
                body: "{}".into(),
            })
        }

        /// How long after the first each request went.
        fn gaps(&self, start: Instant) -> Vec<Duration> {
            let sent = self.sent.lock().unwrap();
            sent.iter().map(|at| *at - start).collect()
        }
    }

    async fn ask(queue: &MarketQueue, stand: &Stand, signed_in: bool) -> Market<StatusCode> {
        match queue.send(signed_in, || stand.reply()).await {
            Ok(Market::Answer(reply)) => Market::Answer(reply.status),
            Ok(Market::Paused(p)) => Market::Paused(p),
            Ok(Market::Unanswered(why)) => Market::Unanswered(why),
            Err(e) => panic!("{e}"),
        }
    }

    #[tokio::test(start_paused = true)]
    async fn signed_in_requests_go_five_seconds_apart_and_up_to_a_second_more() {
        let queue = MarketQueue::new(MarketPace::default());
        let stand = Stand::default();
        let start = Instant::now();

        for _ in 0..4 {
            ask(&queue, &stand, true).await;
        }

        let sent = stand.gaps(start);
        assert_eq!(sent[0], Duration::ZERO, "the first goes at once");
        for pair in sent.windows(2) {
            let gap = pair[1] - pair[0];
            assert!((5 * SECOND..=6 * SECOND).contains(&gap), "{gap:?}");
        }
    }

    #[tokio::test(start_paused = true)]
    async fn signed_out_requests_go_twelve_seconds_apart() {
        let queue = MarketQueue::new(MarketPace::default());
        let stand = Stand::default();
        let start = Instant::now();

        for _ in 0..3 {
            ask(&queue, &stand, false).await;
        }

        assert_eq!(
            stand.gaps(start),
            [Duration::ZERO, 12 * SECOND, 24 * SECOND]
        );
    }

    #[tokio::test(start_paused = true)]
    async fn requests_go_one_at_a_time() {
        let queue = Arc::new(MarketQueue::new(MarketPace::default()));
        let stand = Stand::default();
        let start = Instant::now();

        let together: Vec<_> = (0..3)
            .map(|_| {
                let (queue, stand) = (Arc::clone(&queue), stand.clone());
                tokio::spawn(async move { ask(&queue, &stand, false).await })
            })
            .collect();
        for one in together {
            one.await.unwrap();
        }

        assert_eq!(
            stand.gaps(start),
            [Duration::ZERO, 12 * SECOND, 24 * SECOND]
        );
    }

    #[tokio::test(start_paused = true)]
    async fn a_request_turned_down_pauses_the_market_and_nothing_is_asked_meanwhile() {
        let queue = MarketQueue::new(MarketPace::default());
        let stand = Stand::answering(&[429, 429, 429, 429, 429]);
        let start = Instant::now();

        let Market::Paused(first) = ask(&queue, &stand, true).await else {
            panic!("paused");
        };
        assert_eq!(first.step, 10 * MINUTE);
        tokio::time::sleep(9 * MINUTE).await;
        assert!(matches!(ask(&queue, &stand, true).await, Market::Paused(_)));
        assert_eq!(stand.gaps(start).len(), 1, "nothing asked during the pause");

        // Once it's over, one goes to see; turned down again, it doubles.
        let mut steps = Vec::new();
        for _ in 0..4 {
            let pause = queue.pause().unwrap();
            let left = pause
                .until
                .duration_since(SystemTime::now())
                .unwrap_or_default();
            tokio::time::sleep(left + SECOND).await;
            let Market::Paused(p) = ask(&queue, &stand, true).await else {
                panic!("paused");
            };
            steps.push(p.step);
        }
        assert_eq!(
            steps,
            [20 * MINUTE, 40 * MINUTE, 60 * MINUTE, 60 * MINUTE],
            "20, 40 and 60 minutes, and never longer"
        );
        assert_eq!(stand.gaps(start).len(), 5, "one request each time");
    }

    #[tokio::test(start_paused = true)]
    async fn a_request_that_gets_through_ends_the_pause() {
        let queue = MarketQueue::new(MarketPace::default());
        let stand = Stand::answering(&[429]);
        ask(&queue, &stand, true).await;
        tokio::time::sleep(10 * MINUTE).await;

        assert_eq!(
            ask(&queue, &stand, true).await,
            Market::Answer(StatusCode::OK)
        );
        assert_eq!(queue.pause(), None);

        let stand = Stand::answering(&[429]);
        let Market::Paused(p) = ask(&queue, &stand, true).await else {
            panic!("paused");
        };
        assert_eq!(p.step, 10 * MINUTE, "a new pause starts at 10 minutes");
    }

    #[tokio::test(start_paused = true)]
    async fn a_request_that_goes_unanswered_says_so() {
        let queue = MarketQueue::new(MarketPace::default());

        let answer = queue
            .send(true, || async {
                Err(anyhow!("steamcommunity.com didn't answer (timed out)"))
            })
            .await
            .unwrap();

        assert!(
            matches!(&answer, Market::Unanswered(why) if why == "steamcommunity.com didn't answer (timed out)"),
            "not an error: nothing wrong with what was asked for"
        );
    }

    #[tokio::test(start_paused = true)]
    async fn a_pause_is_over_once_the_systems_clock_says_so() {
        // The computer slept through it: tokio's clock didn't move.
        let queue = MarketQueue::new(MarketPace::default());
        *queue.pause.lock().unwrap() = Some((
            Instant::now() + 10 * MINUTE,
            MarketPause {
                until: SystemTime::now() - SECOND,
                step: 10 * MINUTE,
            },
        ));
        let stand = Stand::default();

        assert_eq!(
            ask(&queue, &stand, true).await,
            Market::Answer(StatusCode::OK)
        );
        assert_eq!(queue.pause(), None);
    }

    #[tokio::test(start_paused = true)]
    async fn a_pause_from_before_a_restart_holds() {
        let queue = MarketQueue::new(MarketPace::default());
        queue.resume(MarketPause {
            until: SystemTime::now() + 40 * MINUTE,
            step: 40 * MINUTE,
        });
        let stand = Stand::answering(&[429]);

        assert!(matches!(ask(&queue, &stand, true).await, Market::Paused(_)));
        assert!(stand.gaps(Instant::now()).is_empty(), "nothing asked");
        tokio::time::sleep(40 * MINUTE + SECOND).await;
        let Market::Paused(p) = ask(&queue, &stand, true).await else {
            panic!("paused");
        };
        assert_eq!(p.step, 60 * MINUTE, "twice 40 minutes, but an hour at most");
    }

    #[tokio::test(start_paused = true)]
    async fn a_server_error_is_asked_once_more_thirty_seconds_later() {
        let queue = MarketQueue::new(MarketPace::default());
        let stand = Stand::answering(&[502]);
        let start = Instant::now();

        assert_eq!(
            ask(&queue, &stand, true).await,
            Market::Answer(StatusCode::OK)
        );
        let sent = stand.gaps(start);
        assert!(sent[1] - sent[0] >= 30 * SECOND, "{sent:?}");

        let stand = Stand::answering(&[500, 503]);
        assert_eq!(
            ask(&queue, &stand, true).await,
            Market::Unanswered(
                "steamcommunity.com's market said 503 Service Unavailable, twice".into()
            ),
            "no answer: nothing wrong with what was asked for"
        );
        let stand = Stand::answering(&[404]);
        let e = queue.send(true, || stand.reply()).await.unwrap_err();
        assert!(e.to_string().ends_with("said 404 Not Found"), "{e}");
    }

    #[test]
    fn the_market_is_asked_as_steams_own_pages_ask_it() {
        assert_eq!(
            search_path(620, false, 0),
            "/market/search/render/?norender=1&appid=753\
             &category_753_Game%5B%5D=tag_app_620\
             &category_753_item_class%5B%5D=tag_item_class_2\
             &category_753_cardborder%5B%5D=tag_cardborder_0\
             &sort_column=name&sort_dir=asc&start=0"
        );
        assert!(
            search_path(220, true, 10)
                .ends_with("tag_cardborder_1&sort_column=name&sort_dir=asc&start=10")
        );
        assert_eq!(
            orderbook_path("620-Chell"),
            "/market/orderbook?q=Load&qp=%5B753%2C%22620-Chell%22%5D",
            "research §1.1's live request"
        );
        assert_eq!(
            orderbook_path("220-Gordon & Alyx (Foil Trading Card)"),
            "/market/orderbook?q=Load&qp=%5B753%2C%22220-Gordon+%26+Alyx+%28Foil+Trading+Card%29%22%5D"
        );
    }

    #[test]
    fn a_page_of_cards_reads_as_the_market_wrote_it() {
        // From a live answer for Portal 2's cards, 2026-09-29, cut short.
        let body = r#"{"success":true,"start":0,"pagesize":10,"total_count":16,
            "searchdata":{"query":"","total_count":16,"pagesize":10},
            "results":[
              {"name":"Chell","hash_name":"620-Chell","sell_listings":4662,"sell_price":6,
               "sell_price_text":"$0.06","app_name":"Steam",
               "asset_description":{"appid":753,"classid":"149749395","type":"Portal 2 Trading Card",
                 "market_name":"Chell","market_hash_name":"620-Chell","commodity":1},
               "sale_price_text":"$0.05"},
              {"name":"Intro (Foil Trading Card)","hash_name":"620-Intro (Foil Trading Card)",
               "sell_listings":97,"sell_price":63,"sell_price_text":"$0.63",
               "asset_description":{"type":"Portal 2 Foil Trading Card"},
               "sale_price_text":"$0.61"}]}"#;

        let page = read_search(body).unwrap();

        assert_eq!((page.total, page.page_size), (16, 10));
        assert_eq!(
            page.listed,
            [
                Listed {
                    hash_name: "620-Chell".into(),
                    name: "Chell".into(),
                    sell_price: 6,
                    sell_price_text: "$0.06".into(),
                    sell_listings: 4662,
                    item_type: "Portal 2 Trading Card".into(),
                },
                Listed {
                    hash_name: "620-Intro (Foil Trading Card)".into(),
                    name: "Intro".into(),
                    sell_price: 63,
                    sell_price_text: "$0.63".into(),
                    sell_listings: 97,
                    item_type: "Portal 2 Foil Trading Card".into(),
                },
            ]
        );
        assert!(read_search(r#"{"success":false}"#).is_err());
        assert!(read_search("<html></html>").is_err());
    }

    #[test]
    fn an_order_book_reads_in_either_shape() {
        // Chell's, 2026-09-29 at 16:43Z, signed out from a GB address.
        let wrapped = r#"{"data":{"success":true,"data":{"amtMaxBuyOrder":4,"amtMinSellOrder":5,
            "eCurrency":2,"cBuyOrders":41763,"cSellOrders":4662,
            "rgCompactBuyOrders":[4,4327,3,37436],"rgCompactSellOrders":[5,2,6,3]}}}"#;
        let bare = r#"{"success":true,"data":{"amtMaxBuyOrder":4,"amtMinSellOrder":5,
            "eCurrency":2,"cBuyOrders":41763,"cSellOrders":4662}}"#;
        let chell = OrderBook {
            lowest_ask: 5,
            highest_bid: 4,
            sell_orders: 4662,
            buy_orders: 41763,
            currency: 2,
        };

        assert_eq!(read_order_book(wrapped).unwrap(), chell);
        assert_eq!(read_order_book(bare).unwrap(), chell);
        assert!(
            read_order_book(r#"{"data":{"success":false}}"#).is_err(),
            "730-SAS, a name the market doesn't know"
        );
        assert!(read_order_book(r#"{"success":false}"#).is_err());
        assert!(
            read_order_book(r#"{"success":true,"data":{"amtMinSellOrder":5}}"#).is_err(),
            "no currency"
        );
    }
}
