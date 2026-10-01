//! Paused-time tier: this session of farming, as the user meets it. Every
//! card that drops is a drop of its own, told at once and named a moment
//! later; the session keeps what was played, the games finished and the
//! first forecast, through pauses, until the user signs out. Every test
//! drives the real `DefaultFarmCardsUseCase` over a fake Steam account, on
//! tokio's paused time.

use std::{any::type_name_of_val, sync::Arc, time::Duration};

use card::{
    AssetId, CardKind, DefaultIdentifyCardsUseCase, DefaultLookAtCardsUseCase,
    DefaultLookAtFoilsUseCase,
};
use farming::{
    DefaultFarmCardsUseCase, FarmCardsUseCase,
    FarmingEvent::{
        self, AllDropped, AskFailed, BuildingHours, ByCardPage, Dropped, FarmingCards, Identified,
        Looked, Stalled, Untold,
    },
    FarmingStatus, FarmingUpdate, Status,
    test_support::FakeSteamAccount,
};
use game::{AppId, DefaultGetLibraryUseCase};
use preferences::{Preferences, test_support::StubGetPreferencesUseCase};
use session::{
    DefaultEndSessionUseCase, DropCard, EndSessionUseCase, Finished, Mode, NewItem, SessionKeeper,
};
use tokio::{sync::mpsc, task::JoinHandle, time::Instant};
use tokio_util::sync::CancellationToken;

const MINUTE: Duration = Duration::from_secs(60);
const HOUR: Duration = Duration::from_secs(60 * 60);

const HEAVY_RAIN: u32 = 960_910;
const HADES: u32 = 1_145_360;

/// Someone farming, who can pause, carry on, and sign out.
struct Player {
    steam: Arc<FakeSteamAccount>,
    prefs: Arc<StubGetPreferencesUseCase>,
    farm: Arc<dyn FarmCardsUseCase>,
    end_session: Arc<dyn EndSessionUseCase>,
    token: CancellationToken,
    tx: mpsc::Sender<FarmingUpdate>,
    updates: mpsc::Receiver<FarmingUpdate>,
    farmer: Option<JoinHandle<()>>,
}

impl Player {
    fn new() -> Self {
        let steam = Arc::new(FakeSteamAccount::new());
        let prefs = Arc::new(StubGetPreferencesUseCase::default());
        let sessions = Arc::new(SessionKeeper::default());
        let (tx, updates) = mpsc::channel(8192);
        Self {
            farm: Arc::new(DefaultFarmCardsUseCase::new(
                Arc::new(DefaultGetLibraryUseCase::new(steam.clone())),
                Arc::new(DefaultLookAtCardsUseCase::new(steam.clone())),
                Arc::new(DefaultLookAtFoilsUseCase::new(steam.clone())),
                Arc::new(DefaultIdentifyCardsUseCase::new(steam.clone())),
                steam.clone(),
                prefs.clone(),
                sessions.clone(),
            )),
            end_session: Arc::new(DefaultEndSessionUseCase::new(sessions)),
            steam,
            prefs,
            token: CancellationToken::new(),
            tx,
            updates,
            farmer: None,
        }
    }

    /// Heavy Rain, 4 hours played, with `left` drops to come, one every
    /// `every`. Its set: Madison and Scott, one each, and three it hasn't.
    fn has_heavy_rain(&self, left: u32, every: Duration) {
        self.steam.add_game(HEAVY_RAIN, 4.0, left, Some(every));
        self.steam.name(HEAVY_RAIN, "Heavy Rain");
        self.steam.set(
            HEAVY_RAIN,
            &[
                ("Ethan", 0),
                ("Carter", 0),
                ("Madison", 1),
                ("Norman", 0),
                ("Scott", 1),
            ],
        );
    }

    /// Skips a game: it's never farmed.
    fn skips(&self, app_id: u32) {
        self.prefs.set(Preferences {
            skipped_games: vec![AppId(app_id)],
            ..Default::default()
        });
    }

    fn starts_farming(&mut self) {
        self.token = CancellationToken::new();
        self.farmer = Some(self.farm.call(self.token.clone(), self.tx.clone()));
    }

    /// Pauses, waits for the farmer to stop, and reads what it said until
    /// then.
    async fn pauses(&mut self) {
        self.token.cancel();
        if let Some(farmer) = self.farmer.take() {
            farmer.await.unwrap();
        }
        while self.updates.try_recv().is_ok() {}
    }

    fn signs_out(&self) {
        self.end_session.call();
    }

    /// Reads what happens until something `that` is about shows up, and
    /// returns it.
    async fn reads(&mut self, that: impl Fn(&FarmingEvent) -> bool) -> FarmingEvent {
        let wait = async {
            loop {
                let update = self.updates.recv().await.expect("the farmer stopped");
                if let FarmingUpdate::Event(e) = update
                    && that(&e)
                {
                    return e;
                }
            }
        };
        tokio::time::timeout(48 * HOUR, wait)
            .await
            .unwrap_or_else(|_| panic!("no {} within two days", type_name_of_val(&that)))
    }

    /// The next status, whatever it says.
    async fn status(&mut self) -> FarmingStatus {
        let wait = async {
            loop {
                let update = self.updates.recv().await.expect("the farmer stopped");
                if let FarmingUpdate::Status(s) = update {
                    return *s;
                }
            }
        };
        tokio::time::timeout(48 * HOUR, wait)
            .await
            .expect("a status within two days")
    }

    /// Reads until the status says `status`, and returns it.
    async fn sees(&mut self, status: Status) -> FarmingStatus {
        loop {
            let s = self.status().await;
            if s.status == status {
                return s;
            }
        }
    }

    /// Lets the farmer run for `d` of tokio's time.
    async fn waits(&self, d: Duration) {
        tokio::time::sleep(d).await;
    }
}

impl Drop for Player {
    fn drop(&mut self) {
        self.token.cancel();
    }
}

/// The farmer started playing: a game on its own, or games together for
/// their hours.
fn playing(e: &FarmingEvent) -> bool {
    matches!(e, FarmingCards { .. } | BuildingHours { .. })
}

fn dropped(e: &FarmingEvent) -> bool {
    matches!(e, Dropped { .. })
}

fn identified(e: &FarmingEvent) -> bool {
    matches!(e, Identified { .. })
}

/// A look at the cards that found no new drop.
fn looked(e: &FarmingEvent) -> bool {
    matches!(e, Looked { .. })
}

/// `card` named as it dropped for `game`: the `copy`th of it held.
fn named(card: &str, game: &str, copy: u32) -> FarmingEvent {
    told(card, CardKind::Normal, game, copy)
}

/// A foil `card` named as it dropped for `game`: the `copy`th foil of it
/// held.
fn named_foil(card: &str, game: &str, copy: u32) -> FarmingEvent {
    told(card, CardKind::Foil, game, copy)
}

fn told(card: &str, kind: CardKind, game: &str, copy: u32) -> FarmingEvent {
    Identified {
        game: game.into(),
        card: card.into(),
        kind,
        copy: Some(copy),
    }
}

#[tokio::test(start_paused = true)]
async fn a_card_that_drops_is_told_at_once_and_named_a_moment_later() {
    let mut player = Player::new();
    player.has_heavy_rain(2, 20 * MINUTE);
    player.steam.will_drop(HEAVY_RAIN, &["Madison"]);
    player.starts_farming();
    assert_eq!(
        player.reads(playing).await,
        FarmingCards {
            game: "Heavy Rain".into(),
            cards_left: 2
        }
    );

    player.waits(20 * MINUTE + Duration::from_secs(5)).await;
    player.steam.announce();

    assert_eq!(
        player.reads(dropped).await,
        Dropped {
            game: "Heavy Rain".into(),
            count: 1,
            left: 1
        }
    );
    let dropped = player.status().await.session.drops;
    assert_eq!(dropped.len(), 1);
    assert_eq!(
        (dropped[0].app_id, &dropped[0].card, dropped[0].copy),
        (AppId(HEAVY_RAIN), &DropCard::Identifying, None),
        "being found out"
    );

    assert_eq!(
        player.reads(identified).await,
        named("Madison", "Heavy Rain", 2)
    );
    let named = player.status().await.session.drops;
    let madison = player.steam.held()[0].clone();
    assert_eq!(named[0].card, DropCard::Identified(madison.clone()));
    assert_eq!(named[0].copy, Some(2), "the set had one already");
    assert!(named[0].is_spare());
    assert_eq!(
        player.steam.describes(),
        [vec![madison.asset_id.0]],
        "Steam was asked about the item it announced"
    );
}

#[tokio::test(start_paused = true)]
async fn two_cards_in_one_look_are_two_drops_numbered_in_turn() {
    let mut player = Player::new();
    player.has_heavy_rain(4, 7 * MINUTE);
    player.steam.will_drop(HEAVY_RAIN, &["Madison", "Madison"]);
    player.starts_farming();
    player.reads(playing).await;

    player.waits(15 * MINUTE).await;
    player.steam.announce();

    assert_eq!(
        player.reads(dropped).await,
        Dropped {
            game: "Heavy Rain".into(),
            count: 2,
            left: 2
        }
    );
    assert_eq!(
        player.reads(identified).await,
        named("Madison", "Heavy Rain", 2)
    );
    assert_eq!(
        player.reads(identified).await,
        named("Madison", "Heavy Rain", 3)
    );
    let drops = player.status().await.session.drops;
    let held = player.steam.held();
    assert_eq!(
        drops
            .iter()
            .map(|d| (d.card.clone(), d.copy))
            .collect::<Vec<_>>(),
        [
            (DropCard::Identified(held[0].clone()), Some(2)),
            (DropCard::Identified(held[1].clone()), Some(3)),
        ],
        "one card, two copies: two drops"
    );
}

#[tokio::test(start_paused = true)]
async fn when_steam_gives_only_a_count_the_card_page_names_the_card() {
    let mut player = Player::new();
    player.has_heavy_rain(4, 7 * MINUTE);
    player.steam.will_drop(HEAVY_RAIN, &["Scott", "Madison"]);
    player.starts_farming();
    player.reads(playing).await;

    player.waits(15 * MINUTE).await;
    player.steam.new_items();

    assert_eq!(
        player.reads(dropped).await,
        Dropped {
            game: "Heavy Rain".into(),
            count: 2,
            left: 2
        }
    );
    assert_eq!(
        player.reads(|e| matches!(e, ByCardPage { .. })).await,
        ByCardPage {
            game: "Heavy Rain".into()
        }
    );
    let first = player.reads(identified).await;
    let second = player.reads(identified).await;
    assert_eq!(
        [first, second],
        [
            named("Madison", "Heavy Rain", 2),
            named("Scott", "Heavy Rain", 2)
        ],
        "in the set's order: the page doesn't say which came first"
    );
    let drops = player.status().await.session.drops;
    assert_eq!(
        drops[0].card,
        DropCard::NameOnly {
            name: "Madison".into()
        }
    );
    assert!(player.steam.describes().is_empty(), "no item to ask about");
}

#[tokio::test(start_paused = true)]
async fn when_steam_cant_say_which_card_the_card_page_does() {
    let mut player = Player::new();
    player.has_heavy_rain(2, 20 * MINUTE);
    player.steam.will_drop(HEAVY_RAIN, &["Scott"]);
    player.steam.cant_describe();
    player.starts_farming();
    player.reads(playing).await;

    player.waits(20 * MINUTE + Duration::from_secs(5)).await;
    player.steam.announce();

    player.reads(dropped).await;
    assert_eq!(
        player.reads(|e| matches!(e, AskFailed { .. })).await,
        AskFailed {
            why: "Steam didn't answer in time".into()
        }
    );
    assert_eq!(
        player.reads(identified).await,
        named("Scott", "Heavy Rain", 2)
    );
}

#[tokio::test(start_paused = true)]
async fn a_card_nothing_can_tell_is_unknown() {
    // A foil: the card page counts only the normal set.
    let mut player = Player::new();
    player.has_heavy_rain(2, 20 * MINUTE);
    player.steam.will_drop_foil(HEAVY_RAIN, "Madison");
    player.starts_farming();
    player.reads(playing).await;

    player.waits(20 * MINUTE + Duration::from_secs(5)).await;
    player.steam.new_items();

    player.reads(dropped).await;
    assert_eq!(
        player.reads(|e| matches!(e, Untold { .. })).await,
        Untold {
            game: "Heavy Rain".into()
        }
    );
    let drops = player.status().await.session.drops;
    assert_eq!((&drops[0].card, drops[0].copy), (&DropCard::Unknown, None));
}

#[tokio::test(start_paused = true)]
async fn foils_are_counted_by_their_own_badge() {
    let mut player = Player::new();
    player.steam.add_game(HADES, 12.0, 3, Some(20 * MINUTE));
    player.steam.name(HADES, "Hades");
    player.steam.set(HADES, &[("Zagreus", 1), ("Thanatos", 3)]);
    player.steam.holds_foils(HADES, &[("Thanatos", 1)]);
    player.steam.will_drop_foil(HADES, "Thanatos");
    player.steam.will_drop_foil(HADES, "Zagreus");
    player.starts_farming();
    player.reads(playing).await;

    player.waits(20 * MINUTE + Duration::from_secs(5)).await;
    player.steam.announce();
    assert_eq!(
        player.reads(identified).await,
        named_foil("Thanatos", "Hades", 2),
        "a foil Thanatos was held already; the set's normal ones don't count"
    );

    player.waits(20 * MINUTE).await;
    player.steam.announce();
    assert_eq!(
        player.reads(identified).await,
        named_foil("Zagreus", "Hades", 1)
    );
    let drops = player.status().await.session.drops;
    assert!(drops[0].is_spare() && !drops[1].is_spare());
}

#[tokio::test(start_paused = true)]
async fn items_steam_says_came_from_another_game_are_kept_for_it() {
    let mut player = Player::new();
    player.has_heavy_rain(2, 20 * MINUTE);
    player.steam.will_drop(HEAVY_RAIN, &["Madison"]);
    player.starts_farming();
    player.reads(playing).await;

    player.waits(10 * MINUTE).await;
    player.steam.announce_items(vec![
        NewItem {
            asset_id: AssetId(99_001),
            app_id: Some(AppId(HADES)),
            gained_at: None,
        },
        NewItem {
            asset_id: AssetId(99_002),
            app_id: None,
            gained_at: None,
        },
    ]);
    player.waits(10 * MINUTE + Duration::from_secs(5)).await;
    player.steam.announce();

    assert_eq!(
        player.reads(identified).await,
        named("Madison", "Heavy Rain", 2)
    );
    assert_eq!(
        player.steam.describes(),
        [vec![99_002, 30_001]],
        "Hades' item waits for a Hades drop; one from who knows where is asked about"
    );
}

#[tokio::test(start_paused = true)]
async fn a_card_that_drops_while_building_hours_is_recorded() {
    let mut player = Player::new();
    player.steam.add_game(HADES, 1.0, 2, Some(20 * MINUTE));
    player.steam.name(HADES, "Hades");
    player.steam.set(HADES, &[("Zagreus", 1), ("Nyx", 0)]);
    player.steam.drops_straight_away(HADES);
    player.steam.will_drop(HADES, &["Zagreus"]);
    player.starts_farming();
    assert_eq!(
        player.reads(playing).await,
        BuildingHours {
            lead: "Hades".into(),
            games: 1
        }
    );

    player.waits(20 * MINUTE + Duration::from_secs(5)).await;
    player.steam.announce();

    assert_eq!(
        player.reads(dropped).await,
        Dropped {
            game: "Hades".into(),
            count: 1,
            left: 1
        }
    );
    assert_eq!(
        player.reads(identified).await,
        named("Zagreus", "Hades", 2),
        "its set was never read, so its card page is, after it"
    );
    let session = player.status().await.session;
    assert_eq!(session.drops[0].copy, Some(2));
    assert_eq!(session.stretches[0].mode, Mode::Hours);
    assert_eq!(session.stretches[0].app_ids, [AppId(HADES)]);
    assert!(
        session.stretches[0].to.is_some(),
        "it stopped to read again"
    );
}

#[tokio::test(start_paused = true)]
async fn a_card_that_drops_while_another_device_plays_is_recorded_after() {
    let mut player = Player::new();
    player.has_heavy_rain(2, HOUR);
    player.steam.will_drop(HEAVY_RAIN, &["Madison"]);
    player.starts_farming();
    assert_eq!(
        player.reads(looked).await,
        Looked {
            game: "Heavy Rain".into(),
            cards_left: 2
        },
        "its set is read first"
    );

    player.steam.block(Some(HEAVY_RAIN));
    let waiting = player.sees(Status::Blocked).await;
    assert!(
        waiting.session.stretches.iter().all(|s| s.to.is_some()),
        "waiting isn't a stretch"
    );
    player.steam.drops_now(HEAVY_RAIN);
    player.steam.announce();
    player.waits(MINUTE).await;
    player.steam.unblock();

    assert_eq!(
        player.reads(dropped).await,
        Dropped {
            game: "Heavy Rain".into(),
            count: 1,
            left: 1
        }
    );
    assert_eq!(
        player.reads(identified).await,
        named("Madison", "Heavy Rain", 2)
    );
}

#[tokio::test(start_paused = true)]
async fn a_card_found_after_another_device_played_is_told_by_its_card_page() {
    let mut player = Player::new();
    player.has_heavy_rain(3, HOUR);
    player.steam.will_drop(HEAVY_RAIN, &["Madison", "Scott"]);
    player.starts_farming();
    player.reads(looked).await;

    // Another device plays it, and Madison drops meanwhile: Steam gives only
    // a count.
    player.steam.block(Some(HEAVY_RAIN));
    player.sees(Status::Blocked).await;
    player.steam.drops_now(HEAVY_RAIN);
    player.steam.new_items();
    player.waits(MINUTE).await;
    player.steam.unblock();

    assert_eq!(
        player.reads(dropped).await,
        Dropped {
            game: "Heavy Rain".into(),
            count: 1,
            left: 2
        }
    );
    assert_eq!(
        player.reads(identified).await,
        named("Madison", "Heavy Rain", 2),
        "its card page is looked at straight away"
    );

    // Scott drops next, and again Steam gives only a count.
    player.steam.drops_now(HEAVY_RAIN);
    player.steam.new_items();
    assert_eq!(
        player.reads(identified).await,
        named("Scott", "Heavy Rain", 2),
        "not Madison again: Madison's copy was counted"
    );
}

#[tokio::test(start_paused = true)]
async fn an_item_steam_announces_late_names_its_own_card_and_no_later_one() {
    let mut player = Player::new();
    player.has_heavy_rain(4, 100 * HOUR);
    player
        .steam
        .will_drop(HEAVY_RAIN, &["Madison", "Scott", "Ethan"]);
    player.starts_farming();
    player.reads(looked).await;
    let item = |asset_id| NewItem {
        asset_id: AssetId(asset_id),
        app_id: Some(AppId(HEAVY_RAIN)),
        gained_at: None,
    };

    // Two cards drop, and Steam has announced only the first when the farmer
    // looks.
    player.steam.drops_now(HEAVY_RAIN);
    player.steam.drops_now(HEAVY_RAIN);
    player.steam.announce_items(vec![item(30_001)]);
    assert_eq!(
        player.reads(identified).await,
        named("Madison", "Heavy Rain", 2)
    );
    assert_eq!(
        player.reads(identified).await,
        named("Scott", "Heavy Rain", 2)
    );

    // Steam announces the second after; then Ethan drops.
    player.steam.announce_items(vec![item(30_002)]);
    player.waits(MINUTE).await;
    player.steam.drops_now(HEAVY_RAIN);
    player.steam.announce_items(vec![item(30_003)]);

    assert_eq!(
        player.reads(identified).await,
        named("Ethan", "Heavy Rain", 1),
        "not a 3rd Scott"
    );
    let drops = player.status().await.session.drops;
    let held = player.steam.held();
    assert_eq!(
        drops
            .iter()
            .map(|d| (d.card.clone(), d.copy))
            .collect::<Vec<_>>(),
        [
            (DropCard::Identified(held[0].clone()), Some(2)),
            (DropCard::Identified(held[1].clone()), Some(2)),
            (DropCard::Identified(held[2].clone()), Some(1)),
        ],
        "Scott's item named the drop the card page had told, once it came"
    );
}

#[tokio::test(start_paused = true)]
async fn the_session_counts_from_its_first_read_and_marks_each_game_finished() {
    let mut player = Player::new();
    player.steam.add_game(620, 5.0, 2, Some(20 * MINUTE));
    player.steam.add_game(440, 1.0, 3, None);
    player.steam.add_game(220, 9.0, 0, None);
    player.starts_farming();

    let first = player.sees(Status::Farming).await.session;
    assert_eq!(first.drops_left_at_start, Some(5));
    assert_eq!(first.games_at_start, Some(2), "Game 220 has none left");
    assert!(first.drops.is_empty() && first.finished.is_empty());

    assert_eq!(
        player.reads(|e| matches!(e, AllDropped { .. })).await,
        AllDropped {
            game: "Game 620".into()
        }
    );
    let session = player.status().await.session;
    assert_eq!(session.drops.len(), 2);
    assert_eq!(
        session.finished,
        [Finished {
            app_id: AppId(620),
            at: session.drops[1].at
        }],
        "finished when its last drop was seen"
    );
    assert_eq!(
        session.drops_left_at_start,
        Some(5),
        "still what it started from"
    );
}

#[tokio::test(start_paused = true)]
async fn the_session_counts_only_the_games_it_will_farm() {
    let mut player = Player::new();
    player.steam.add_game(620, 5.0, 2, Some(20 * MINUTE));
    player.steam.add_game(730, 350.0, 2, None);
    player.steam.add_game(2_861_720, 0.0, 3, None);
    player.skips(730);
    player.starts_farming();

    let session = player.sees(Status::Farming).await.session;

    assert_eq!(
        (session.drops_left_at_start, session.games_at_start),
        (Some(2), Some(1)),
        "a skipped game, and a sale's badge, aren't farmed"
    );
}

#[tokio::test(start_paused = true)]
async fn the_first_forecast_is_made_when_the_second_drop_lands() {
    let mut player = Player::new();
    player.steam.add_game(620, 5.0, 4, Some(20 * MINUTE));
    player.starts_farming();
    player.reads(playing).await;

    player.waits(20 * MINUTE + Duration::from_secs(5)).await;
    player.steam.new_items();
    player.reads(dropped).await;
    assert_eq!(player.status().await.session.first_forecast, None);

    player.waits(20 * MINUTE).await;
    player.steam.new_items();
    player.reads(dropped).await;
    let forecast = player
        .status()
        .await
        .session
        .first_forecast
        .expect("a forecast at the second drop");
    assert!(!forecast.assumed, "two drops to learn from");
    assert!(forecast.band.is_some());
    assert_eq!(forecast.per_game.len(), 1);
}

#[tokio::test(start_paused = true)]
async fn the_session_carries_on_across_a_pause() {
    let mut player = Player::new();
    player.has_heavy_rain(3, 20 * MINUTE);
    player.steam.will_drop(HEAVY_RAIN, &["Madison"]);
    player.starts_farming();
    player.reads(playing).await;
    player.waits(20 * MINUTE + Duration::from_secs(5)).await;
    player.steam.announce();
    player.reads(identified).await;
    let before = player.status().await.session;

    player.pauses().await;
    player.waits(2 * HOUR).await;
    player.starts_farming();

    let after = player.sees(Status::Farming).await.session;
    assert_eq!(after.started_at, before.started_at, "the same session");
    assert_eq!(after.drops, before.drops, "and its drops");
    assert_eq!(after.drops_left_at_start, Some(3));
    let stretches: Vec<(Mode, bool)> = after
        .stretches
        .iter()
        .map(|s| (s.mode, s.to.is_some()))
        .collect();
    assert_eq!(
        stretches,
        [(Mode::Cards, true), (Mode::Cards, false)],
        "the pause stopped one stretch, and farming again started another"
    );
}

#[tokio::test(start_paused = true)]
async fn after_a_pause_the_first_word_is_the_session_going_on() {
    let mut player = Player::new();
    player.has_heavy_rain(3, 20 * MINUTE);
    player.steam.will_drop(HEAVY_RAIN, &["Madison"]);
    player.starts_farming();
    player.reads(playing).await;
    player.waits(20 * MINUTE + Duration::from_secs(5)).await;
    player.steam.announce();
    player.reads(identified).await;
    let before = player.status().await;

    player.pauses().await;
    player.starts_farming();

    let first = player.status().await;
    assert_eq!(first.status, Status::Checking, "reading the badges");
    assert_eq!(
        (first.session.started_at, &first.session.drops),
        (before.session.started_at, &before.session.drops),
        "the same session, its drops and all"
    );
    assert_eq!(
        first.library.game(AppId(HEAVY_RAIN)).map(|g| g.drops),
        before.library.game(AppId(HEAVY_RAIN)).map(|g| g.drops)
    );
    assert_eq!(first.order, [AppId(HEAVY_RAIN)]);
}

#[tokio::test(start_paused = true)]
async fn a_pause_while_a_card_is_told_stops_at_once() {
    let mut player = Player::new();
    player.has_heavy_rain(3, 20 * MINUTE);
    player.steam.will_drop(HEAVY_RAIN, &["Madison"]);
    player.steam.describes_after(Duration::from_secs(15));
    player.starts_farming();
    player.reads(playing).await;
    player.waits(20 * MINUTE + Duration::from_secs(5)).await;
    player.steam.announce();
    player.reads(dropped).await;

    // Steam is asked which card it was, and takes its time.
    let paused = Instant::now();
    player.pauses().await;

    assert!(
        paused.elapsed() < Duration::from_secs(1),
        "stopped {:?} after the pause",
        paused.elapsed()
    );
    assert!(player.steam.playing().is_empty(), "and stopped playing");
}

#[tokio::test(start_paused = true)]
async fn a_game_set_aside_stays_behind_through_a_pause() {
    let mut player = Player::new();
    player.steam.add_game(1, 5.0, 2, None);
    player.steam.add_game(2, 5.0, 4, Some(3 * HOUR));
    player.starts_farming();
    assert_eq!(
        player.reads(playing).await,
        FarmingCards {
            game: "Game 1".into(),
            cards_left: 2
        }
    );
    assert_eq!(
        player.reads(|e| matches!(e, Stalled { .. })).await,
        Stalled {
            game: "Game 1".into(),
            after: 10 * HOUR,
            for_good: false
        }
    );
    let aside = player.sees(Status::Farming).await.set_aside;
    assert_eq!(
        aside
            .iter()
            .map(|s| (s.app_id.0, s.times))
            .collect::<Vec<_>>(),
        [(1, 1)]
    );

    player.pauses().await;
    player.starts_farming();

    assert_eq!(
        player.reads(playing).await,
        FarmingCards {
            game: "Game 2".into(),
            cards_left: 4
        },
        "Game 1 is still behind the others"
    );
    let status = player.sees(Status::Farming).await;
    assert_eq!(status.set_aside, aside);
    assert_eq!(status.order, [AppId(2), AppId(1)]);
}

#[tokio::test(start_paused = true)]
async fn signing_out_ends_the_session() {
    let mut player = Player::new();
    player.has_heavy_rain(3, 20 * MINUTE);
    player.starts_farming();
    player.reads(playing).await;
    player.waits(20 * MINUTE + Duration::from_secs(5)).await;
    player.steam.new_items();
    player.reads(dropped).await;
    let before = player.status().await.session;

    player.pauses().await;
    player.signs_out();
    player.starts_farming();

    let after = player.sees(Status::Farming).await.session;
    assert!(after.drops.is_empty(), "a new session");
    assert!(after.started_at >= before.started_at);
    assert_eq!(after.drops_left_at_start, Some(2), "counted from now");
    assert_eq!(after.stretches.len(), 1);
}

#[tokio::test(start_paused = true)]
async fn the_last_card_is_looked_for_every_five_minutes() {
    let mut player = Player::new();
    player.steam.add_game(620, 5.0, 2, Some(20 * MINUTE));
    player.starts_farming();

    let farming = player.sees(Status::Farming).await;
    assert_eq!(
        farming.look_every,
        Some(15 * MINUTE + Duration::from_secs(15))
    );

    player.reads(dropped).await;
    let dropped = Instant::now();
    assert_eq!(
        player.reads(looked).await,
        Looked {
            game: "Game 620".into(),
            cards_left: 1
        }
    );
    assert_eq!(dropped.elapsed(), 5 * MINUTE);
    let last_card = player.sees(Status::Farming).await;
    assert_eq!(last_card.look_every, Some(5 * MINUTE));
}

#[tokio::test(start_paused = true)]
async fn nothing_is_looked_at_while_building_hours() {
    let mut player = Player::new();
    player.steam.add_game(620, 1.0, 2, Some(20 * MINUTE));
    player.starts_farming();

    let building = player.sees(Status::Farming).await;
    assert_eq!(building.mode, Some(Mode::Hours));
    assert_eq!(building.look_every, None);
}
