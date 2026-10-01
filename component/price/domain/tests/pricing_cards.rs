//! Paused-time tier: prices as the user meets them, over days that pass in
//! moments. The watcher and the other use cases run for real over a fake
//! market on tokio's paused time, which a real connection to the market
//! couldn't run on. Tests read what the user would: the prices on screen,
//! and the log. The acceptance tier, through the price component, is in
//! price-di.

use std::{sync::Arc, time::Duration};

use chrono::{DateTime, TimeDelta, Utc};
use money::{Currency, Money};
use price::{
    Basis, Clock, DefaultGetPriceSettingsUseCase, DefaultGetPricesUseCase, DefaultGetWalletUseCase,
    DefaultKeepPricesUpToDateUseCase, DefaultLookUpOffersUseCase, DefaultRefreshPricesUseCase,
    DefaultSetBasisUseCase, DefaultSetGamesToPriceUseCase, GetPriceSettingsUseCase,
    GetPricesUseCase, GetWalletUseCase, HeldCard, KeepPricesUpToDateUseCase, LookUpOffersUseCase,
    MarketPause, Price, PriceError, PriceEvent, RefreshPricesUseCase, SetBasisUseCase,
    SetGamesToPriceUseCase, held_value,
    test_support::{self, FakePriceRepository},
};
use steam_library::AppId;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

const MINUTE: Duration = Duration::from_secs(60);
const HOUR: Duration = Duration::from_secs(60 * 60);

const HEAVY_RAIN: u32 = 960_910;
const HADES: u32 = 1_145_360;
const CELESTE: u32 = 504_230;

/// How long from `at` until `next`, as the screens count it.
fn between(at: DateTime<Utc>, next: Option<DateTime<Utc>>) -> Duration {
    (next.expect("a next round") - at)
        .to_std()
        .unwrap_or_default()
}

/// Someone farming with prices on screen.
struct Player {
    market: Arc<FakePriceRepository>,
    clock: Clock,
    want: Arc<dyn SetGamesToPriceUseCase>,
    prices: Arc<dyn GetPricesUseCase>,
    refresh: Arc<dyn RefreshPricesUseCase>,
    offers: Arc<dyn LookUpOffersUseCase>,
    token: CancellationToken,
    events: Option<mpsc::Receiver<PriceEvent>>,
}

impl Player {
    fn new() -> Self {
        let clock = test_support::clock_from(test_support::session_start());
        let market = Arc::new(FakePriceRepository::new(Arc::clone(&clock)));
        market.lists(
            HEAVY_RAIN,
            &[
                ("Ethan", 5),
                ("Carter", 4),
                ("Madison", 5),
                ("Norman", 6),
                ("Scott", 4),
            ],
            &[("Ethan", 42), ("Madison", 60)],
        );
        market.lists(HADES, &[("Zagreus", 8), ("Nyx", 9)], &[("Thanatos", 62)]);
        market.lists(CELESTE, &[("Madeline", 7), ("Badeline", 6)], &[]);
        Self {
            want: Arc::new(DefaultSetGamesToPriceUseCase::new(market.clone())),
            prices: Arc::new(DefaultGetPricesUseCase::new(market.clone())),
            refresh: Arc::new(DefaultRefreshPricesUseCase::new(
                market.clone(),
                Arc::clone(&clock),
            )),
            offers: Arc::new(DefaultLookUpOffersUseCase::new(
                market.clone(),
                Arc::clone(&clock),
            )),
            market,
            clock,
            token: CancellationToken::new(),
            events: None,
        }
    }

    fn starts_farming(&mut self) {
        let pricing =
            DefaultKeepPricesUpToDateUseCase::new(self.market.clone(), Arc::clone(&self.clock));
        let (tx, rx) = mpsc::channel(1024);
        drop(pricing.call(self.token.clone(), tx));
        self.events = Some(rx);
    }

    /// The games shown, most urgent first.
    fn is_shown(&self, games: &[u32]) {
        self.want.call(games.iter().copied().map(AppId).collect());
    }

    /// Reads the log until a line of the kind wanted shows up.
    async fn reads(&mut self, kind: impl Fn(&PriceEvent) -> bool) -> PriceEvent {
        let mut lines = self.reads_up_to(kind).await;
        lines.pop().expect("the line wanted")
    }

    /// Reads the log up to and including a line of the kind wanted.
    async fn reads_up_to(&mut self, kind: impl Fn(&PriceEvent) -> bool) -> Vec<PriceEvent> {
        let rx = self.events.as_mut().expect("the watcher started");
        let wait = async {
            let mut lines = Vec::new();
            loop {
                let event = rx.recv().await.expect("the watcher stopped");
                let wanted = kind(&event);
                lines.push(event);
                if wanted {
                    return lines;
                }
            }
        };
        tokio::time::timeout(2 * 24 * HOUR, wait)
            .await
            .expect("no such line within two days")
    }

    async fn waits(&self, d: Duration) {
        tokio::time::sleep(d).await;
    }

    fn now(&self) -> DateTime<Utc> {
        (self.clock)()
    }

    /// What the screen shows for a card of a game's set.
    fn sees(&self, app_id: u32, name: &str, foil: bool) -> Price {
        self.prices
            .call()
            .sets
            .get(&AppId(app_id))
            .map_or(Price::Pending, |set| set.price(name, foil))
    }

    /// Just the lowest listing, in pence.
    fn listed_at(&self, app_id: u32, name: &str, foil: bool) -> Option<i64> {
        match self.sees(app_id, name, foil) {
            Price::Known(quote) => quote.ask.map(|ask| ask.minor),
            _ => None,
        }
    }
}

impl Drop for Player {
    fn drop(&mut self) {
        self.token.cancel();
    }
}

fn after(start: DateTime<Utc>, d: Duration) -> DateTime<Utc> {
    start + TimeDelta::from_std(d).unwrap()
}

#[tokio::test(start_paused = true)]
async fn the_farming_game_is_priced_first_then_the_others_in_order() {
    let mut player = Player::new();
    player.is_shown(&[HEAVY_RAIN, HADES, CELESTE]);
    player.starts_farming();

    let all = player
        .reads(|e| matches!(e, PriceEvent::AllPriced { .. }))
        .await;

    let PriceEvent::AllPriced {
        games,
        next_round,
        at,
    } = all
    else {
        unreachable!("read above");
    };
    assert_eq!(games, 3);
    assert_eq!(
        next_round,
        Some(after(test_support::session_start(), 6 * HOUR)),
        "what the screens word: \"the next round is at 15:14\""
    );
    assert_eq!(between(at, next_round), 6 * HOUR, "\"in 6h\"");
    assert_eq!(
        player.market.looked_up(),
        [
            (HEAVY_RAIN, false),
            (HEAVY_RAIN, true),
            (HADES, false),
            (HADES, true),
            (CELESTE, false),
            (CELESTE, true),
        ],
        "each game's normal cards, then its foils"
    );
    assert_eq!(player.listed_at(HEAVY_RAIN, "Madison", false), Some(5));
    assert_eq!(player.listed_at(HEAVY_RAIN, "Madison", true), Some(60));
    assert_eq!(player.listed_at(HADES, "Thanatos", true), Some(62));
    assert_eq!(
        player.sees(HEAVY_RAIN, "Scott", true),
        Price::NoMarket,
        "nobody is selling a foil Scott"
    );
    assert_eq!(
        player.sees(620, "Chell", false),
        Price::Pending,
        "not shown"
    );
}

#[tokio::test(start_paused = true)]
async fn prices_are_looked_up_again_once_they_are_six_hours_old() {
    let mut player = Player::new();
    player.is_shown(&[HEAVY_RAIN]);
    player.starts_farming();
    player
        .reads(|e| matches!(e, PriceEvent::AllPriced { .. }))
        .await;

    player.waits(6 * HOUR - MINUTE).await;
    assert_eq!(player.market.looked_up().len(), 2, "still fresh");

    player.waits(2 * MINUTE).await;
    assert_eq!(player.market.looked_up().len(), 4, "looked up again");
    let again = player.market.set_lookups()[2].at;
    assert_eq!(again, after(test_support::session_start(), 6 * HOUR));
}

#[tokio::test(start_paused = true)]
async fn a_game_that_starts_to_matter_is_priced_within_moments() {
    let mut player = Player::new();
    player.is_shown(&[HEAVY_RAIN]);
    player.starts_farming();
    player
        .reads(|e| matches!(e, PriceEvent::AllPriced { .. }))
        .await;

    // A card dropped for Hades: it's shown near the top now.
    player.is_shown(&[HEAVY_RAIN, HADES]);
    player.waits(Duration::from_secs(31)).await;

    assert_eq!(
        player.market.looked_up()[2..],
        [(HADES, false), (HADES, true)]
    );
    assert_eq!(
        player.market.looked_up().len(),
        4,
        "Heavy Rain is still fresh"
    );
}

#[tokio::test(start_paused = true)]
async fn a_game_whose_card_dropped_is_priced_again_if_its_prices_are_over_an_hour_old() {
    let mut player = Player::new();
    player.is_shown(&[HEAVY_RAIN]);
    player.starts_farming();
    player
        .reads(|e| matches!(e, PriceEvent::AllPriced { .. }))
        .await;

    player.waits(30 * MINUTE).await;
    player
        .refresh
        .call(AppId(HEAVY_RAIN))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        player.market.looked_up().len(),
        2,
        "priced half an hour ago"
    );

    player.waits(31 * MINUTE).await;
    player
        .refresh
        .call(AppId(HEAVY_RAIN))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        player.market.looked_up()[2..],
        [(HEAVY_RAIN, false), (HEAVY_RAIN, true)]
    );
    let set = &player.prices.call().sets[&AppId(HEAVY_RAIN)];
    assert_eq!(set.fetched_at, player.now(), "the new prices are shown");
}

#[tokio::test(start_paused = true)]
async fn lookups_wait_while_steam_has_paused_them() {
    let mut player = Player::new();
    let start = player.now();
    player.market.turns_down(2);
    player.is_shown(&[HEAVY_RAIN, HADES]);
    player.starts_farming();

    let first = player
        .reads(|e| matches!(e, PriceEvent::Paused { .. }))
        .await;
    assert_eq!(
        first,
        PriceEvent::Paused {
            pause: MarketPause {
                until: after(start, 10 * MINUTE),
                step: 10 * MINUTE,
            },
            again: false,
        }
    );
    let again = player
        .reads(|e| matches!(e, PriceEvent::Paused { .. }))
        .await;
    assert!(
        matches!(again, PriceEvent::Paused { pause, again: true } if pause.step == 20 * MINUTE),
        "{again:?}"
    );
    player.reads(|e| *e == PriceEvent::Resumed).await;
    player
        .reads(|e| matches!(e, PriceEvent::AllPriced { .. }))
        .await;

    assert_eq!(
        player.market.turned_down(),
        [start, after(start, 10 * MINUTE)],
        "nothing asked during a pause: one lookup when it ends, to see"
    );
    let lookups = player.market.set_lookups();
    assert_eq!(lookups.len(), 4);
    assert_eq!(lookups[0].at, after(start, 30 * MINUTE));
    assert_eq!(player.listed_at(HADES, "Nyx", false), Some(9));
}

#[tokio::test(start_paused = true)]
async fn a_pause_from_before_a_restart_is_waited_out() {
    let mut player = Player::new();
    let start = player.now();
    player.market.paused(MarketPause {
        until: after(start, 40 * MINUTE),
        step: 40 * MINUTE,
    });
    player.is_shown(&[HEAVY_RAIN]);
    player.starts_farming();

    let paused = player
        .reads(|e| matches!(e, PriceEvent::Paused { .. }))
        .await;
    assert!(
        matches!(paused, PriceEvent::Paused { pause, again: false } if pause.step == 40 * MINUTE),
        "{paused:?}"
    );
    player
        .reads(|e| matches!(e, PriceEvent::AllPriced { .. }))
        .await;

    assert!(player.market.turned_down().is_empty(), "nothing was asked");
    assert_eq!(player.market.set_lookups()[0].at, after(start, 40 * MINUTE));
}

#[tokio::test(start_paused = true)]
async fn a_pause_that_outlasts_the_clock_after_a_sleep_is_told_once() {
    let mut player = Player::new();
    let start = player.now();
    // Over by the clock, but the market's queue didn't count the sleep, and
    // turns the next three asks away.
    player.market.holds_pause(
        MarketPause {
            until: start - TimeDelta::minutes(1),
            step: 40 * MINUTE,
        },
        3,
    );
    player.is_shown(&[HEAVY_RAIN]);
    player.starts_farming();

    let lines = player
        .reads_up_to(|e| matches!(e, PriceEvent::AllPriced { .. }))
        .await;

    assert!(
        matches!(
            lines[..],
            [
                PriceEvent::Paused { .. },
                PriceEvent::Resumed,
                PriceEvent::AllPriced { .. }
            ]
        ),
        "the pause told once: {lines:?}"
    );
    assert_eq!(
        player.market.set_lookups()[0].at,
        after(start, 3 * Duration::from_secs(30)),
        "asked again a tick at a time, not at once"
    );
}

#[tokio::test(start_paused = true)]
async fn a_lookup_whose_answer_cant_be_used_is_tried_again_a_day_later() {
    let mut player = Player::new();
    let start = player.now();
    player.market.fails(HADES);
    player.is_shown(&[HADES, HEAVY_RAIN]);
    player.starts_farming();

    let failed = player
        .reads(|e| matches!(e, PriceEvent::Failed { .. }))
        .await;
    assert_eq!(
        failed,
        PriceEvent::Failed {
            app_id: AppId(HADES),
            why: "steamcommunity.com's market said 502 Bad Gateway, twice".into(),
        }
    );
    let all = player
        .reads(|e| matches!(e, PriceEvent::AllPriced { .. }))
        .await;
    assert!(
        matches!(all, PriceEvent::AllPriced { games: 2, next_round, at } if between(at, next_round) == 6 * HOUR),
        "{all:?}"
    );
    assert_eq!(
        player.sees(HADES, "Nyx", false),
        Price::Failed {
            retry_at: after(start, 24 * HOUR)
        },
        "shown as a failed lookup, never as nothing"
    );

    player.waits(23 * HOUR).await;
    let hades = |p: &Player| {
        p.market
            .looked_up()
            .iter()
            .filter(|(app, _)| *app == HADES)
            .count()
    };
    assert_eq!(hades(&player), 1, "not before a day has passed");
    player.waits(2 * HOUR).await;
    assert_eq!(hades(&player), 2);
}

#[tokio::test(start_paused = true)]
async fn a_market_that_cant_be_asked_is_asked_again_soon_and_nothing_is_failed() {
    let mut player = Player::new();
    let start = player.now();
    player
        .market
        .cant_be_asked(3, "couldn't sign on to Steam: no network");
    player.is_shown(&[CELESTE, HADES]);
    player.starts_farming();

    let lines = player
        .reads_up_to(|e| matches!(e, PriceEvent::AllPriced { .. }))
        .await;

    let waits: Vec<(&str, Duration)> = lines
        .iter()
        .filter_map(|l| match l {
            PriceEvent::Unanswered { why, wait, .. } => Some((why.as_str(), *wait)),
            _ => None,
        })
        .collect();
    let no_network = "couldn't sign on to Steam: no network";
    assert_eq!(
        waits,
        [
            (no_network, MINUTE),
            (no_network, 2 * MINUTE),
            (no_network, 4 * MINUTE),
        ],
        "sooner than a day, and never over and over"
    );
    assert!(
        !lines.iter().any(|l| matches!(l, PriceEvent::Failed { .. })),
        "nothing failed"
    );
    assert_eq!(
        player.market.set_lookups()[0],
        test_support::SetLookup {
            app_id: CELESTE,
            foil: false,
            at: after(start, 7 * MINUTE),
        },
        "the same game first, once the market could be asked"
    );
    assert_eq!(player.listed_at(HADES, "Nyx", false), Some(9));
}

#[tokio::test(start_paused = true)]
async fn prices_from_before_stay_while_the_market_cant_be_asked() {
    let player = Player::new();
    let eight_hours_ago = player.now() - TimeDelta::hours(8);
    player.market.knows(vec![test_support::set_prices(
        CELESTE,
        &[("Badeline", 6)],
        &[],
        eight_hours_ago,
    )]);
    player.market.cant_be_asked(2, "no network");

    let refreshed = player.refresh.call(AppId(CELESTE)).await.unwrap();
    let offers = player
        .offers
        .call(vec!["504230-Badeline".into()])
        .await
        .unwrap();

    assert_eq!(refreshed, Err(PriceError::Unanswered));
    assert_eq!(offers, Err(PriceError::Unanswered));
    assert_eq!(
        player.listed_at(CELESTE, "Badeline", false),
        Some(6),
        "stale, and still shown"
    );
    assert!(
        player.prices.call().offers.is_empty(),
        "no order book taken as failed"
    );
}

#[tokio::test(start_paused = true)]
async fn prices_from_before_a_failed_lookup_are_still_shown() {
    let mut player = Player::new();
    let start = player.now();
    let eight_hours_ago = start - TimeDelta::hours(8);
    player.market.knows(vec![test_support::set_prices(
        CELESTE,
        &[("Badeline", 6)],
        &[],
        eight_hours_ago,
    )]);
    player.market.fails(CELESTE);
    player.is_shown(&[CELESTE]);
    player.starts_farming();

    player
        .reads(|e| matches!(e, PriceEvent::Failed { .. }))
        .await;

    assert_eq!(player.listed_at(CELESTE, "Badeline", false), Some(6));
    let Price::Known(badeline) = player.sees(CELESTE, "Badeline", false) else {
        unreachable!("listed above");
    };
    assert!(badeline.is_stale(player.now()), "dim, with its age");
    assert_eq!(
        player.sees(CELESTE, "Madeline", false),
        Price::Failed {
            retry_at: after(start, 24 * HOUR)
        }
    );
}

#[tokio::test(start_paused = true)]
async fn best_offers_are_looked_up_once_for_each_card_held() {
    let player = Player::new();
    player.market.offers("960910-Madison", 5, 4);

    player
        .offers
        .call(vec![
            "960910-Madison".into(),
            "960910-Madison".into(),
            "1145360-Zagreus".into(),
        ])
        .await
        .unwrap()
        .unwrap();

    assert_eq!(
        player.market.offer_lookups(),
        ["960910-Madison", "1145360-Zagreus"],
        "each card once"
    );
    let book = player.prices.call();
    assert_eq!(book.offers["1145360-Zagreus"].price, Price::NoMarket);
    let madison = HeldCard {
        market_hash_name: Some("960910-Madison".into()),
        ..HeldCard::named(AppId(HEAVY_RAIN), "Madison", false)
    };
    let sold_now = held_value(
        &[madison],
        0,
        &book,
        Basis::Instant,
        &test_support::pounds(),
        player.now(),
    );
    assert_eq!(
        sold_now.total,
        Money::new(2, Currency::GBP),
        "a 4p offer pays 2p"
    );

    player.waits(29 * MINUTE).await;
    player
        .offers
        .call(vec!["960910-Madison".into()])
        .await
        .unwrap()
        .unwrap();
    assert_eq!(player.market.offer_lookups().len(), 2, "still fresh");
    player.waits(2 * MINUTE).await;
    player
        .offers
        .call(vec!["960910-Madison".into()])
        .await
        .unwrap()
        .unwrap();
    assert_eq!(player.market.offer_lookups().len(), 3, "half an hour old");
}

#[tokio::test(start_paused = true)]
async fn an_order_book_nobody_is_on_is_looked_up_again_only_after_half_an_hour() {
    let player = Player::new();

    player
        .offers
        .call(vec!["1145360-Zagreus".into()])
        .await
        .unwrap()
        .unwrap();
    player.waits(29 * MINUTE).await;
    player
        .offers
        .call(vec!["1145360-Zagreus".into()])
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        player.market.offer_lookups().len(),
        1,
        "nobody buying or selling is an answer too"
    );

    player.waits(2 * MINUTE).await;
    player
        .offers
        .call(vec!["1145360-Zagreus".into()])
        .await
        .unwrap()
        .unwrap();
    assert_eq!(player.market.offer_lookups().len(), 2);
}

#[tokio::test(start_paused = true)]
async fn offers_wait_while_steam_has_paused_lookups() {
    let player = Player::new();
    let pause = MarketPause {
        until: after(player.now(), 20 * MINUTE),
        step: 20 * MINUTE,
    };
    player.market.paused(pause);

    let asked = player
        .offers
        .call(vec!["960910-Madison".into()])
        .await
        .unwrap();

    assert_eq!(asked, Err(PriceError::Paused(pause)));
    assert!(player.market.offer_lookups().is_empty());
    assert_eq!(
        player.refresh.call(AppId(HEAVY_RAIN)).await.unwrap(),
        Err(PriceError::Paused(pause))
    );
}

#[tokio::test]
async fn money_is_shown_at_list_prices_until_the_user_picks_another_basis() {
    let clock = test_support::clock_from(test_support::session_start());
    let market = Arc::new(FakePriceRepository::new(clock));
    let settings = DefaultGetPriceSettingsUseCase::new(market.clone());
    let basis = DefaultSetBasisUseCase::new(market.clone());
    assert_eq!(settings.call().basis, Basis::List);

    basis.call(Basis::Net).unwrap();
    assert_eq!(settings.call().basis, Basis::Net);

    market.disk_full();
    assert_eq!(basis.call(Basis::Instant), Err(PriceError::Unavailable));
    assert_eq!(settings.call().basis, Basis::Net, "nothing changed");
}

#[tokio::test]
async fn the_wallet_is_whatever_steam_last_said() {
    let clock = test_support::clock_from(test_support::session_start());
    let market = Arc::new(FakePriceRepository::new(clock));
    let wallet = DefaultGetWalletUseCase::new(market.clone());
    market.wallet_is(None);
    assert_eq!(wallet.call(), None, "not signed on yet");

    market.wallet_is(Some(test_support::pounds()));
    let pounds = wallet.call().unwrap();
    assert_eq!(pounds.currency, Currency::GBP);
    assert_eq!(pounds.seller_gets(62), 55, "Valve's fees");
}
