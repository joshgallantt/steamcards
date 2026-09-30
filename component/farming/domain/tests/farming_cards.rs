//! Acceptance tier: the farmer as the user meets it. Every test drives the
//! real `DefaultFarmCardsUseCase` over an in-memory Steam, on paused time, and reads only
//! what the user would read — the log and the status line.

use std::{sync::Arc, time::Duration};

use card::{DefaultIdentifyCardsUseCase, DefaultLookAtCardsUseCase, DefaultLookAtFoilsUseCase};
use farming::{
    DefaultFarmCardsUseCase, EventKind, FarmCardsUseCase, FarmingEvent, FarmingStatus, Status,
    test_support::InMemorySteam,
};
use game::{AppId, DefaultReadLibraryUseCase};
use preferences::{Preferences, test_support::StubGetPreferencesUseCase};
use session::Mode;
use tokio::{sync::mpsc, time::Instant};
use tokio_util::sync::CancellationToken;

const MINUTE: Duration = Duration::from_secs(60);
const HOUR: Duration = Duration::from_secs(60 * 60);

/// Someone with a Steam library, leaving the farmer running.
struct Player {
    steam: Arc<InMemorySteam>,
    prefs: Arc<StubGetPreferencesUseCase>,
    token: CancellationToken,
    events: Option<mpsc::Receiver<FarmingEvent>>,
    farmer: Option<tokio::task::JoinHandle<()>>,
}

impl Player {
    fn new() -> Self {
        Self {
            steam: Arc::new(InMemorySteam::new()),
            prefs: Arc::default(),
            token: CancellationToken::new(),
            events: None,
            farmer: None,
        }
    }

    fn wants(&self, p: Preferences) {
        self.prefs.set(p);
    }

    fn starts_farming(&mut self) {
        let farm = DefaultFarmCardsUseCase::new(
            Arc::new(DefaultReadLibraryUseCase::new(self.steam.clone())),
            Arc::new(DefaultLookAtCardsUseCase::new(self.steam.clone())),
            Arc::new(DefaultLookAtFoilsUseCase::new(self.steam.clone())),
            Arc::new(DefaultIdentifyCardsUseCase::new(self.steam.clone())),
            self.steam.clone(),
            self.prefs.clone(),
            Arc::default(),
        );
        let (tx, rx) = mpsc::channel(8192);
        self.farmer = Some(farm.call(self.token.clone(), tx));
        self.events = Some(rx);
    }

    /// Reads the log until a line of `kind` shows up, and returns it.
    async fn reads(&mut self, kind: EventKind) -> String {
        let rx = self.events.as_mut().unwrap();
        let wait = async {
            loop {
                let e = rx.recv().await.expect("the farmer stopped");
                if e.kind == kind && !e.message.is_empty() {
                    return e.message;
                }
            }
        };
        tokio::time::timeout(Duration::from_secs(48 * 60 * 60), wait)
            .await
            .unwrap_or_else(|_| panic!("no {kind:?} line within two days"))
    }

    /// Reads until the status says `status`, and returns it.
    async fn sees(&mut self, status: Status) -> FarmingStatus {
        let rx = self.events.as_mut().unwrap();
        let wait = async {
            loop {
                let e = rx.recv().await.expect("the farmer stopped");
                if let Some(s) = e.status.filter(|s| s.status == status) {
                    return s;
                }
            }
        };
        tokio::time::timeout(Duration::from_secs(48 * 60 * 60), wait)
            .await
            .unwrap_or_else(|_| panic!("no {status:?} status within two days"))
    }

    /// Reads until the status says farming waits for another device playing
    /// `app_id`, and returns it.
    async fn sees_waiting_for(&mut self, app_id: u32) -> FarmingStatus {
        loop {
            let waiting = self.sees(Status::Blocked).await;
            if waiting.blocked_by == Some(AppId(app_id)) {
                return waiting;
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

#[tokio::test(start_paused = true)]
async fn a_game_with_three_hours_is_farmed_alone_until_every_card_drops() {
    let mut player = Player::new();
    player.steam.drops_only_alone();
    player.steam.add_game(620, 5.0, 3, Some(30 * MINUTE));
    player.steam.add_game(440, 1.0, 2, Some(30 * MINUTE));
    player.starts_farming();

    assert_eq!(
        player.reads(EventKind::Playing).await,
        "Farming Game 620 — 3 cards to drop"
    );
    assert_eq!(
        player.reads(EventKind::Dropped).await,
        "A card dropped for Game 620 — 2 to go"
    );
    player.reads(EventKind::Dropped).await;
    assert_eq!(
        player.reads(EventKind::Dropped).await,
        "A card dropped for Game 620 — that's all of them"
    );
    assert_eq!(
        player.reads(EventKind::Playing).await,
        "Playing Game 440 until it has 3 hours, when its cards can start dropping"
    );
    assert_eq!(player.steam.played(), [vec![620], vec![440]]);
}

#[tokio::test(start_paused = true)]
async fn games_short_of_three_hours_are_played_together_until_one_gets_there() {
    let mut player = Player::new();
    player.steam.drops_only_alone();
    player.steam.add_game(30, 0.0, 2, Some(30 * MINUTE));
    player.steam.add_game(10, 2.5, 2, Some(30 * MINUTE));
    player.steam.add_game(20, 1.0, 2, Some(30 * MINUTE));
    player.starts_farming();

    let started = Instant::now();
    assert_eq!(
        player.reads(EventKind::Playing).await,
        "Playing 3 games together until Game 10 has 3 hours, when its cards can start dropping"
    );
    let building = player.sees(Status::Farming).await;
    assert_eq!(building.mode, Some(Mode::Hours));
    assert_eq!(
        building.playing,
        [AppId(10), AppId(20), AppId(30)],
        "the most hours first"
    );

    assert_eq!(
        player.reads(EventKind::Info).await,
        "Game 10 has 3 hours now: its cards can drop."
    );
    let waited = started.elapsed();
    assert!(
        waited >= 30 * MINUTE && waited < 32 * MINUTE,
        "half an hour to go from 2.5 hours, counted here: {waited:?}"
    );
    assert_eq!(
        player.reads(EventKind::Playing).await,
        "Farming Game 10 — 2 cards to drop"
    );
    assert_eq!(player.steam.played()[..2], [vec![10, 20, 30], vec![10]]);
}

#[tokio::test(start_paused = true)]
async fn the_users_first_choice_is_farmed_first() {
    let mut player = Player::new();
    player.steam.add_game(620, 5.0, 3, Some(30 * MINUTE));
    player.steam.add_game(440, 8.0, 1, Some(30 * MINUTE));
    player.wants(Preferences {
        priority_games: vec![AppId(620)],
        ..Default::default()
    });
    player.starts_farming();

    assert_eq!(
        player.reads(EventKind::Playing).await,
        "Farming Game 620 — 3 cards to drop"
    );
}

#[tokio::test(start_paused = true)]
async fn a_first_choice_short_of_three_hours_leads_the_group() {
    let mut player = Player::new();
    player.steam.add_game(620, 5.0, 3, Some(30 * MINUTE));
    player.steam.add_game(440, 1.0, 2, Some(30 * MINUTE));
    player.steam.add_game(220, 2.0, 2, Some(30 * MINUTE));
    player.wants(Preferences {
        priority_games: vec![AppId(440)],
        ..Default::default()
    });
    player.starts_farming();

    player.reads(EventKind::Playing).await;
    assert_eq!(
        player.steam.played(),
        [vec![440, 220]],
        "Game 620 waits its turn"
    );
}

#[tokio::test(start_paused = true)]
async fn a_skipped_game_is_never_played() {
    let mut player = Player::new();
    player.steam.add_game(620, 5.0, 1, Some(30 * MINUTE));
    player.steam.add_game(440, 5.0, 1, Some(30 * MINUTE));
    player.wants(Preferences {
        skipped_games: vec![AppId(620)],
        ..Default::default()
    });
    player.starts_farming();

    let idle = player.sees(Status::Idle).await;
    assert_eq!(idle.note, "every game with cards left is skipped");
    assert_eq!(player.steam.played(), [vec![440]]);
}

#[tokio::test(start_paused = true)]
async fn only_priority_stops_after_the_priorities() {
    let mut player = Player::new();
    player.steam.add_game(620, 5.0, 1, Some(30 * MINUTE));
    player.steam.add_game(440, 5.0, 1, Some(30 * MINUTE));
    player.wants(Preferences {
        priority_games: vec![AppId(620)],
        only_priority: true,
        ..Default::default()
    });
    player.starts_farming();

    let idle = player.sees(Status::Idle).await;
    assert_eq!(
        idle.note,
        "\"only priority\" is on, and your priority games are done"
    );
    assert_eq!(player.steam.played(), [vec![620]]);
    assert!(player.steam.stops() >= 1, "nothing to play: signed off");
}

#[tokio::test(start_paused = true)]
async fn playing_elsewhere_pauses_farming_until_a_minute_after_it_stops() {
    let mut player = Player::new();
    player.steam.add_game(620, 5.0, 5, Some(HOUR));
    player.starts_farming();
    player.reads(EventKind::Playing).await;

    player.steam.block(Some(730));
    let blocked = player.sees(Status::Blocked).await;
    assert_eq!(blocked.blocked_by, Some(AppId(730)));

    player.waits(2 * HOUR).await;
    assert_eq!(
        player.steam.played().len(),
        1,
        "nothing played while blocked"
    );

    player.steam.unblock();
    let unblocked = Instant::now();
    assert_eq!(
        player.reads(EventKind::Info).await,
        "Playing elsewhere stopped: farming carries on in a minute."
    );
    player.reads(EventKind::Playing).await;
    assert!(unblocked.elapsed() >= MINUTE, "a minute's grace first");
    assert_eq!(player.steam.played(), [vec![620], vec![620]]);
}

#[tokio::test(start_paused = true)]
async fn another_device_taking_over_is_waited_out_signed_on_without_playing() {
    let mut player = Player::new();
    player.steam.add_game(620, 5.0, 5, Some(HOUR));
    player.starts_farming();
    player.reads(EventKind::Playing).await;

    // A game started on another device, whose Steam client closed
    // steamcards' game to let it play.
    player.steam.take_over();
    player.steam.block(Some(730));
    assert_eq!(
        player.reads(EventKind::Info).await,
        "Another device took over playing: farming waits until it stops."
    );
    player.sees_waiting_for(730).await;

    player.waits(2 * HOUR).await;
    assert_eq!(
        player.steam.played().len(),
        1,
        "nothing played meanwhile: Steam would sign steamcards off"
    );
    assert_eq!(
        player.steam.sign_ons(),
        2,
        "signed on again once, to hear when it stops"
    );
    assert_eq!(player.steam.reads(), 1, "nor were the badges read again");

    player.steam.unblock();
    let unblocked = Instant::now();
    assert_eq!(
        player.reads(EventKind::Info).await,
        "Playing elsewhere stopped: farming carries on in a minute."
    );
    player.reads(EventKind::Playing).await;
    assert!(unblocked.elapsed() >= MINUTE, "a minute's grace first");
    assert_eq!(player.steam.played(), [vec![620], vec![620]]);
}

#[tokio::test(start_paused = true)]
async fn after_a_takeover_the_other_devices_game_is_given_minutes_to_start() {
    let mut player = Player::new();
    player.steam.add_game(620, 5.0, 5, Some(HOUR));
    player.starts_farming();
    player.reads(EventKind::Playing).await;

    player.steam.take_over();
    let waiting = player.sees(Status::Blocked).await;
    assert_eq!(waiting.blocked_by, None, "nothing plays there yet");
    let wait = waiting.next_look.map(|at| at - chrono::Utc::now());
    assert!(
        wait.is_some_and(
            |w| w > chrono::TimeDelta::minutes(4) && w <= chrono::TimeDelta::minutes(5)
        ),
        "it says when farming carries on: {wait:?}"
    );

    // Its game takes three minutes to start.
    player.waits(3 * MINUTE).await;
    player.steam.block(Some(730));
    player.sees_waiting_for(730).await;
    player.waits(HOUR).await;
    assert_eq!(player.steam.played().len(), 1, "nothing played meanwhile");

    player.steam.unblock();
    player.reads(EventKind::Playing).await;
    assert_eq!(player.steam.played(), [vec![620], vec![620]]);
}

#[tokio::test(start_paused = true)]
async fn after_a_takeover_with_no_game_started_farming_carries_on_in_five_minutes() {
    let mut player = Player::new();
    player.steam.add_game(620, 5.0, 5, Some(HOUR));
    player.starts_farming();
    player.reads(EventKind::Playing).await;

    player.steam.take_over();
    let taken = Instant::now();

    player.reads(EventKind::Playing).await;
    assert!(taken.elapsed() >= 5 * MINUTE);
    assert_eq!(player.steam.played(), [vec![620], vec![620]]);
}

#[tokio::test(start_paused = true)]
async fn farming_that_starts_while_another_device_plays_plays_nothing_until_it_stops() {
    let mut player = Player::new();
    player.steam.add_game(620, 5.0, 5, Some(HOUR));
    player.steam.block(Some(730));
    player.starts_farming();

    assert_eq!(
        player.reads(EventKind::Info).await,
        "Another device is playing: farming waits until it stops."
    );
    player.sees_waiting_for(730).await;
    player.waits(HOUR).await;
    assert!(
        player.steam.played().is_empty(),
        "nothing played: Steam would sign steamcards off"
    );
    assert_eq!(player.steam.sign_ons(), 1);

    player.steam.unblock();
    player.reads(EventKind::Playing).await;
    assert_eq!(player.steam.played(), [vec![620]]);
}

#[tokio::test(start_paused = true)]
async fn a_game_played_on_another_device_is_named_when_its_in_the_library() {
    let mut player = Player::new();
    player.steam.add_game(620, 5.0, 5, Some(HOUR));
    player.steam.add_game(440, 1.0, 2, Some(HOUR));
    player.steam.name(440, "Team Fortress 2");
    player.starts_farming();
    player.reads(EventKind::Playing).await;

    player.steam.block(Some(440));

    assert_eq!(
        player.reads(EventKind::Info).await,
        "Team Fortress 2 is being played on another device: farming waits until it stops."
    );
}

#[tokio::test(start_paused = true)]
async fn a_new_item_is_looked_at_straight_away() {
    let mut player = Player::new();
    player.steam.add_game(620, 5.0, 3, Some(20 * MINUTE));
    player.starts_farming();
    player.reads(EventKind::Playing).await;
    let started = Instant::now();

    player.waits(20 * MINUTE + Duration::from_secs(5)).await;
    player.steam.new_items();

    assert_eq!(
        player.reads(EventKind::Dropped).await,
        "A card dropped for Game 620 — 2 to go"
    );
    assert!(
        started.elapsed() < 21 * MINUTE,
        "not at the next quarter-hour look"
    );
}

#[tokio::test(start_paused = true)]
async fn a_game_that_drops_nothing_for_ten_hours_waits_behind_the_others() {
    let mut player = Player::new();
    player.steam.add_game(1, 5.0, 2, None);
    player.steam.add_game(2, 5.0, 2, Some(30 * MINUTE));
    player.starts_farming();

    assert_eq!(
        player.reads(EventKind::Playing).await,
        "Farming Game 1 — 2 cards to drop"
    );
    assert_eq!(
        player.reads(EventKind::Warning).await,
        "No card from Game 1 in 10 hours — trying the others first"
    );
    assert_eq!(
        player.reads(EventKind::Playing).await,
        "Farming Game 2 — 2 cards to drop"
    );
}

#[tokio::test(start_paused = true)]
async fn appearing_online_follows_the_preference() {
    let mut player = Player::new();
    player.steam.add_game(620, 5.0, 5, Some(HOUR));
    player.starts_farming();
    player.reads(EventKind::Playing).await;
    assert_eq!(
        player.steam.plays(),
        [(vec![620], false)],
        "offline by default"
    );

    player.wants(Preferences {
        appear_online: true,
        ..Default::default()
    });
    player.waits(MINUTE).await;

    assert_eq!(
        player.steam.plays(),
        [(vec![620], false), (vec![620], true)]
    );
}

#[tokio::test(start_paused = true)]
async fn badges_that_cant_be_read_are_tried_again_in_five_minutes() {
    let mut player = Player::new();
    player.steam.add_game(620, 5.0, 5, Some(HOUR));
    player.steam.go_down(true);
    player.starts_farming();

    assert_eq!(
        player.reads(EventKind::Error).await,
        "Couldn't read your badges: steamcommunity.com didn't answer (503 Service Unavailable)"
    );
    let again = player.sees(Status::Error).await.next_look;
    assert!(
        again.is_some_and(|at| at - chrono::Utc::now() > chrono::TimeDelta::minutes(4)),
        "it says when it tries again: {again:?}"
    );
    let failed = Instant::now();
    player.steam.go_down(false);
    player.reads(EventKind::Playing).await;
    assert!(failed.elapsed() >= 5 * MINUTE);
    assert!(
        player.steam.played().len() == 1,
        "never taken for \"no drops left\""
    );
}

#[tokio::test(start_paused = true)]
async fn a_lost_connection_is_tried_again_after_a_minute() {
    let mut player = Player::new();
    player.steam.add_game(620, 5.0, 5, Some(HOUR));
    player.starts_farming();
    player.reads(EventKind::Playing).await;

    player.steam.lose("the connection to Steam dropped");
    assert_eq!(
        player.reads(EventKind::Warning).await,
        "the connection to Steam dropped — trying again in a minute"
    );
    let status = player.sees(Status::Error).await;
    assert_eq!(status.note, "the connection to Steam dropped");
    let wait = status.next_look.map(|at| at - chrono::Utc::now());
    assert!(
        wait.is_some_and(
            |w| w > chrono::TimeDelta::seconds(50) && w <= chrono::TimeDelta::seconds(60)
        ),
        "it says when it tries again: {wait:?}"
    );
    let lost = Instant::now();
    player.reads(EventKind::Playing).await;
    assert!(lost.elapsed() >= MINUTE);
}

#[tokio::test(start_paused = true)]
async fn another_session_taking_over_stops_farming() {
    let mut player = Player::new();
    player.steam.add_game(620, 5.0, 5, Some(HOUR));
    player.starts_farming();
    player.reads(EventKind::Playing).await;

    player.steam.replace();

    assert!(
        player
            .reads(EventKind::Error)
            .await
            .starts_with("Another session signed in")
    );
    player.farmer.take().unwrap().await.unwrap();
    assert!(player.steam.stops() >= 1);
    assert_eq!(
        player.steam.played().len(),
        1,
        "it didn't knock the other one off"
    );
}

#[tokio::test(start_paused = true)]
async fn a_new_first_choice_takes_over_within_moments() {
    let mut player = Player::new();
    player.steam.add_game(1, 5.0, 5, Some(HOUR));
    player.steam.add_game(2, 5.0, 5, Some(HOUR));
    player.starts_farming();
    assert_eq!(
        player.reads(EventKind::Playing).await,
        "Farming Game 1 — 5 cards to drop"
    );

    player.wants(Preferences {
        priority_games: vec![AppId(2)],
        ..Default::default()
    });

    assert_eq!(
        player.reads(EventKind::Switched).await,
        "Moving on from Game 1: something is ranked higher now"
    );
    assert_eq!(
        player.reads(EventKind::Playing).await,
        "Farming Game 2 — 5 cards to drop"
    );
}

#[tokio::test(start_paused = true)]
async fn nothing_to_farm_says_why() {
    let mut player = Player::new();
    player.steam.add_game(620, 5.0, 0, None);
    player.starts_farming();

    let idle = player.sees(Status::Idle).await;
    assert_eq!(idle.note, "every card has dropped");
    assert!(idle.next_look.is_some(), "it says when it'll look again");
    assert!(player.steam.played().is_empty());
}

#[tokio::test(start_paused = true)]
async fn sale_event_badges_are_never_played() {
    let mut player = Player::new();
    player.steam.add_game(2_861_720, 0.0, 3, Some(HOUR));
    player.starts_farming();

    let idle = player.sees(Status::Idle).await;
    assert_eq!(idle.note, "Steam isn't dropping cards for the games left");
    assert!(player.steam.played().is_empty());
}
