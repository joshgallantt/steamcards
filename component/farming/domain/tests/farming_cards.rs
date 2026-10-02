//! Paused-time tier: the farmer as the user meets it, over hours of play
//! that pass in moments. Every test drives the real `DefaultFarmCardsUseCase`
//! over a fake Steam account on tokio's paused time, which a real connection
//! to Steam couldn't run on, and reads only what the screens are told: what
//! happened, and where farming stands. The words are the screens'.

use std::{any::type_name_of_val, sync::Arc, time::Duration};

use chrono::{TimeDelta, Utc};
use farming::{
    DefaultFarmCardsUseCase, FarmCardsUseCase,
    FarmingEvent::{
        self, BuildingHours, ChoicesChanged, Dropped, ElsewhereStopped, FarmingCards, HoursBuilt,
        MovedOn, PlayedElsewhere, Stalled, TakenOver, WentWrong,
    },
    FarmingStatus, FarmingUpdate, NothingToFarm, Status, Trouble,
    test_support::{FakeSteamAccount, farming_over},
};
use game::AppId;
use preferences::{Preferences, test_support::StubGetPreferencesUseCase};
use session::Mode;
use tokio::{sync::mpsc, time::Instant};
use tokio_util::sync::CancellationToken;

const MINUTE: Duration = Duration::from_secs(60);
const HOUR: Duration = Duration::from_secs(60 * 60);

/// Someone with a Steam library, leaving the farmer running.
struct Player {
    steam: Arc<FakeSteamAccount>,
    prefs: Arc<StubGetPreferencesUseCase>,
    token: CancellationToken,
    updates: Option<mpsc::Receiver<FarmingUpdate>>,
    farmer: Option<tokio::task::JoinHandle<()>>,
}

impl Player {
    fn new() -> Self {
        Self {
            steam: Arc::new(FakeSteamAccount::new()),
            prefs: Arc::default(),
            token: CancellationToken::new(),
            updates: None,
            farmer: None,
        }
    }

    fn wants(&self, p: Preferences) {
        self.prefs.set(p);
    }

    fn starts_farming(&mut self) {
        let farm = DefaultFarmCardsUseCase::new(farming_over(
            &self.steam,
            self.prefs.clone(),
            Arc::default(),
        ));
        let (tx, rx) = mpsc::channel(8192);
        self.farmer = Some(farm.call(self.token.clone(), tx));
        self.updates = Some(rx);
    }

    /// Reads what happens until something `that` is about shows up, and
    /// returns it.
    async fn reads(&mut self, that: impl Fn(&FarmingEvent) -> bool) -> FarmingEvent {
        let rx = self.updates.as_mut().unwrap();
        let wait = async {
            loop {
                let update = rx.recv().await.expect("the farmer stopped");
                if let FarmingUpdate::Event(e) = update
                    && that(&e)
                {
                    return e;
                }
            }
        };
        tokio::time::timeout(Duration::from_secs(48 * 60 * 60), wait)
            .await
            .unwrap_or_else(|_| panic!("no {} within two days", type_name_of_val(&that)))
    }

    /// Reads until the status says `status`, and returns it.
    async fn sees(&mut self, status: Status) -> FarmingStatus {
        let rx = self.updates.as_mut().unwrap();
        let wait = async {
            loop {
                let update = rx.recv().await.expect("the farmer stopped");
                if let FarmingUpdate::Status(s) = update
                    && s.status == status
                {
                    return *s;
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

/// The farmer started playing: a game on its own, or games together for
/// their hours.
fn playing(e: &FarmingEvent) -> bool {
    matches!(e, FarmingCards { .. } | BuildingHours { .. })
}

fn dropped(e: &FarmingEvent) -> bool {
    matches!(e, Dropped { .. })
}

/// Farming waits for another device, or carries on after it.
fn elsewhere(e: &FarmingEvent) -> bool {
    matches!(
        e,
        PlayedElsewhere { .. } | TakenOver | ElsewhereStopped { .. }
    )
}

fn went_wrong(e: &FarmingEvent) -> bool {
    matches!(e, WentWrong { .. })
}

#[tokio::test(start_paused = true)]
async fn a_game_with_three_hours_is_farmed_alone_until_every_card_drops() {
    let mut player = Player::new();
    player.steam.drops_only_alone();
    player.steam.add_game(620, 5.0, 3, Some(30 * MINUTE));
    player.steam.add_game(440, 1.0, 2, Some(30 * MINUTE));
    player.starts_farming();

    assert_eq!(
        player.reads(playing).await,
        FarmingCards {
            game: "Game 620".into(),
            cards_left: 3
        }
    );
    assert_eq!(
        player.reads(dropped).await,
        Dropped {
            game: "Game 620".into(),
            count: 1,
            left: 2
        }
    );
    player.reads(dropped).await;
    assert_eq!(
        player.reads(dropped).await,
        Dropped {
            game: "Game 620".into(),
            count: 1,
            left: 0
        }
    );
    assert_eq!(
        player.reads(playing).await,
        BuildingHours {
            lead: "Game 440".into(),
            games: 1,
            hours: 3
        }
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
        player.reads(playing).await,
        BuildingHours {
            lead: "Game 10".into(),
            games: 3,
            hours: 3
        }
    );
    let building = player.sees(Status::Farming).await;
    assert_eq!(building.mode, Some(Mode::Hours));
    assert_eq!(
        building.playing,
        [AppId(10), AppId(20), AppId(30)],
        "the most hours first"
    );

    assert_eq!(
        player.reads(|e| matches!(e, HoursBuilt { .. })).await,
        HoursBuilt {
            game: "Game 10".into(),
            hours: 3
        }
    );
    let waited = started.elapsed();
    assert!(
        waited >= 30 * MINUTE && waited < 32 * MINUTE,
        "half an hour to go from 2.5 hours, counted here: {waited:?}"
    );
    assert_eq!(
        player.reads(playing).await,
        FarmingCards {
            game: "Game 10".into(),
            cards_left: 2
        }
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
        player.reads(playing).await,
        FarmingCards {
            game: "Game 620".into(),
            cards_left: 3
        }
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

    player.reads(playing).await;
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
    assert_eq!(idle.nothing_to_farm, Some(NothingToFarm::AllSkipped));
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
    assert_eq!(idle.nothing_to_farm, Some(NothingToFarm::PrioritiesDone));
    assert_eq!(player.steam.played(), [vec![620]]);
    assert!(player.steam.stops() >= 1, "nothing to play: signed off");
}

#[tokio::test(start_paused = true)]
async fn a_private_game_is_left_out_unless_the_user_farms_it_too() {
    let mut player = Player::new();
    player.steam.add_game(620, 5.0, 1, Some(30 * MINUTE));
    player.steam.add_game(440, 5.0, 1, Some(30 * MINUTE));
    player.steam.private(620);
    player.starts_farming();

    let idle = player.sees(Status::Idle).await;
    assert_eq!(
        idle.nothing_to_farm,
        Some(NothingToFarm::HeldBack {
            private: true,
            refundable_until: None
        })
    );
    assert_eq!(player.steam.played(), [vec![440]]);

    player.wants(Preferences {
        skip_private: false,
        ..Default::default()
    });
    assert_eq!(
        player.reads(playing).await,
        FarmingCards {
            game: "Game 620".into(),
            cards_left: 1
        },
        "turned off, it's farmed: in moments, not hours"
    );
}

#[tokio::test(start_paused = true)]
async fn a_game_steam_would_still_refund_waits_until_it_wouldnt() {
    let mut player = Player::new();
    player.steam.add_game(620, 0.5, 2, Some(30 * MINUTE));
    let bought = Utc::now() - TimeDelta::days(14) + TimeDelta::hours(4);
    player.steam.bought(620, bought);
    player.starts_farming();

    let idle = player.sees(Status::Idle).await;
    let ends = bought + TimeDelta::days(14);
    assert_eq!(
        idle.nothing_to_farm,
        Some(NothingToFarm::HeldBack {
            private: false,
            refundable_until: Some(ends)
        })
    );
    let next = idle.next_look.expect("a time to look again");
    assert!(
        (next - ends).num_seconds().abs() < 60,
        "looks again as Steam stops refunding it, not 8 hours on: {next} for {ends}"
    );
    assert!(player.steam.played().is_empty(), "never played");
}

#[tokio::test(start_paused = true)]
async fn on_an_account_steam_doesnt_hold_back_every_game_is_farmed_alone() {
    let mut player = Player::new();
    player.steam.drops_only_alone();
    player.steam.add_game(30, 0.0, 1, Some(30 * MINUTE));
    player.steam.add_game(10, 0.5, 2, Some(30 * MINUTE));
    player.steam.drops_straight_away(30);
    player.steam.drops_straight_away(10);
    player.wants(Preferences {
        hours_before_drops: 0,
        ..Default::default()
    });
    player.starts_farming();

    assert_eq!(
        player.reads(playing).await,
        FarmingCards {
            game: "Game 30".into(),
            cards_left: 1
        },
        "fewest drops left first"
    );
    assert_eq!(
        player.reads(playing).await,
        FarmingCards {
            game: "Game 10".into(),
            cards_left: 2
        }
    );
    assert_eq!(player.steam.played()[..2], [vec![30], vec![10]]);
}

#[tokio::test(start_paused = true)]
async fn needing_more_hours_than_a_game_has_builds_them_first() {
    let mut player = Player::new();
    player.steam.add_game(620, 4.0, 3, Some(HOUR));
    player.starts_farming();
    player.reads(playing).await;

    player.wants(Preferences {
        hours_before_drops: 5,
        ..Default::default()
    });

    assert_eq!(
        player
            .reads(|e| matches!(e, ChoicesChanged | MovedOn { .. }))
            .await,
        ChoicesChanged,
        "not moved on from: it builds its hours"
    );
    assert_eq!(
        player.reads(playing).await,
        BuildingHours {
            lead: "Game 620".into(),
            games: 1,
            hours: 5
        }
    );
}

#[tokio::test(start_paused = true)]
async fn playing_elsewhere_pauses_farming_until_a_minute_after_it_stops() {
    let mut player = Player::new();
    player.steam.add_game(620, 5.0, 5, Some(HOUR));
    player.starts_farming();
    player.reads(playing).await;

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
        player.reads(elsewhere).await,
        ElsewhereStopped {
            carry_on_in: MINUTE
        }
    );
    player.reads(playing).await;
    assert!(unblocked.elapsed() >= MINUTE, "a minute's grace first");
    assert_eq!(player.steam.played(), [vec![620], vec![620]]);
}

#[tokio::test(start_paused = true)]
async fn another_device_taking_over_is_waited_out_signed_on_without_playing() {
    let mut player = Player::new();
    player.steam.add_game(620, 5.0, 5, Some(HOUR));
    player.starts_farming();
    player.reads(playing).await;

    // A game started on another device, whose Steam client closed
    // steamcards' game to let it play.
    player.steam.take_over();
    player.steam.block(Some(730));
    assert_eq!(player.reads(elsewhere).await, TakenOver);
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
        player.reads(elsewhere).await,
        ElsewhereStopped {
            carry_on_in: MINUTE
        }
    );
    player.reads(playing).await;
    assert!(unblocked.elapsed() >= MINUTE, "a minute's grace first");
    assert_eq!(player.steam.played(), [vec![620], vec![620]]);
}

#[tokio::test(start_paused = true)]
async fn after_a_takeover_the_other_devices_game_is_given_minutes_to_start() {
    let mut player = Player::new();
    player.steam.add_game(620, 5.0, 5, Some(HOUR));
    player.starts_farming();
    player.reads(playing).await;

    player.steam.take_over();
    let waiting = player.sees(Status::Blocked).await;
    assert_eq!(waiting.blocked_by, None, "nothing plays there yet");
    let wait = waiting.next_look.map(|at| at - Utc::now());
    assert!(
        wait.is_some_and(|w| w > TimeDelta::minutes(4) && w <= TimeDelta::minutes(5)),
        "it says when farming carries on: {wait:?}"
    );

    // Its game takes three minutes to start.
    player.waits(3 * MINUTE).await;
    player.steam.block(Some(730));
    player.sees_waiting_for(730).await;
    player.waits(HOUR).await;
    assert_eq!(player.steam.played().len(), 1, "nothing played meanwhile");

    player.steam.unblock();
    player.reads(playing).await;
    assert_eq!(player.steam.played(), [vec![620], vec![620]]);
}

#[tokio::test(start_paused = true)]
async fn after_a_takeover_with_no_game_started_farming_carries_on_in_five_minutes() {
    let mut player = Player::new();
    player.steam.add_game(620, 5.0, 5, Some(HOUR));
    player.starts_farming();
    player.reads(playing).await;

    player.steam.take_over();
    let taken = Instant::now();

    player.reads(playing).await;
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
        player.reads(elsewhere).await,
        PlayedElsewhere { game: None }
    );
    player.sees_waiting_for(730).await;
    player.waits(HOUR).await;
    assert!(
        player.steam.played().is_empty(),
        "nothing played: Steam would sign steamcards off"
    );
    assert_eq!(player.steam.sign_ons(), 1);

    player.steam.unblock();
    player.reads(playing).await;
    assert_eq!(player.steam.played(), [vec![620]]);
}

#[tokio::test(start_paused = true)]
async fn a_game_played_on_another_device_is_named_when_its_in_the_library() {
    let mut player = Player::new();
    player.steam.add_game(620, 5.0, 5, Some(HOUR));
    player.steam.add_game(440, 1.0, 2, Some(HOUR));
    player.steam.name(440, "Team Fortress 2");
    player.starts_farming();
    player.reads(playing).await;

    player.steam.block(Some(440));

    assert_eq!(
        player.reads(elsewhere).await,
        PlayedElsewhere {
            game: Some("Team Fortress 2".into())
        }
    );
}

#[tokio::test(start_paused = true)]
async fn a_new_item_is_looked_at_straight_away() {
    let mut player = Player::new();
    player.steam.add_game(620, 5.0, 3, Some(20 * MINUTE));
    player.starts_farming();
    player.reads(playing).await;
    let started = Instant::now();

    player.waits(20 * MINUTE + Duration::from_secs(5)).await;
    player.steam.new_items();

    assert_eq!(
        player.reads(dropped).await,
        Dropped {
            game: "Game 620".into(),
            count: 1,
            left: 2
        }
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
    assert_eq!(
        player.reads(playing).await,
        FarmingCards {
            game: "Game 2".into(),
            cards_left: 2
        }
    );
}

#[tokio::test(start_paused = true)]
async fn appearing_online_follows_the_preference() {
    let mut player = Player::new();
    player.steam.add_game(620, 5.0, 5, Some(HOUR));
    player.starts_farming();
    player.reads(playing).await;
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
async fn asked_to_shake_drops_loose_the_game_is_stopped_and_played_again_every_5_minutes() {
    let mut player = Player::new();
    player.steam.add_game(620, 5.0, 5, Some(HOUR));
    player.wants(Preferences {
        restart_games: true,
        ..Default::default()
    });
    player.starts_farming();
    player.reads(playing).await;

    player.waits(11 * MINUTE).await;

    assert_eq!(
        player.steam.played(),
        [vec![620], vec![], vec![620], vec![], vec![620]],
        "stopped for a moment, then played again, twice"
    );
}

#[tokio::test(start_paused = true)]
async fn the_game_being_farmed_is_never_restarted_unless_asked() {
    let mut player = Player::new();
    player.steam.add_game(620, 5.0, 5, Some(HOUR));
    player.starts_farming();
    player.reads(playing).await;

    player.waits(11 * MINUTE).await;

    assert_eq!(player.steam.played(), [vec![620]]);
}

#[tokio::test(start_paused = true)]
async fn badges_that_cant_be_read_are_tried_again_in_five_minutes() {
    let mut player = Player::new();
    player.steam.add_game(620, 5.0, 5, Some(HOUR));
    player.steam.go_down(true);
    player.starts_farming();

    assert_eq!(
        player.reads(went_wrong).await,
        WentWrong {
            trouble: Trouble::BadgesUnread(
                "steamcommunity.com didn't answer (503 Service Unavailable)".into()
            ),
            again_in: Some(5 * MINUTE)
        }
    );
    let again = player.sees(Status::Error).await.next_look;
    assert!(
        again.is_some_and(|at| at - Utc::now() > TimeDelta::minutes(4)),
        "it says when it tries again: {again:?}"
    );
    let failed = Instant::now();
    player.steam.go_down(false);
    player.reads(playing).await;
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
    player.reads(playing).await;

    player.steam.lose("the connection to Steam dropped");
    assert_eq!(
        player.reads(went_wrong).await,
        WentWrong {
            trouble: Trouble::Lost("the connection to Steam dropped".into()),
            again_in: Some(MINUTE)
        }
    );
    let status = player.sees(Status::Error).await;
    assert_eq!(
        status.trouble,
        Some(Trouble::Lost("the connection to Steam dropped".into()))
    );
    let wait = status.next_look.map(|at| at - Utc::now());
    assert!(
        wait.is_some_and(|w| w > TimeDelta::seconds(50) && w <= TimeDelta::seconds(60)),
        "it says when it tries again: {wait:?}"
    );
    let lost = Instant::now();
    player.reads(playing).await;
    assert!(lost.elapsed() >= MINUTE);
}

#[tokio::test(start_paused = true)]
async fn another_session_taking_over_stops_farming() {
    let mut player = Player::new();
    player.steam.add_game(620, 5.0, 5, Some(HOUR));
    player.starts_farming();
    player.reads(playing).await;

    player.steam.replace();

    assert_eq!(
        player.reads(went_wrong).await,
        WentWrong {
            trouble: Trouble::Replaced,
            again_in: None
        }
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
async fn another_session_taking_over_as_the_badges_are_read_stops_farming() {
    let mut player = Player::new();
    player.steam.add_game(620, 5.0, 5, Some(HOUR));
    player.steam.replace();
    player.starts_farming();

    assert_eq!(
        player.reads(went_wrong).await,
        WentWrong {
            trouble: Trouble::Replaced,
            again_in: None
        },
        "said as it is, not as badges that couldn't be read"
    );
    player.farmer.take().unwrap().await.unwrap();
    assert_eq!(
        player.steam.reads(),
        1,
        "not read again: signing on to would knock the other one off"
    );
    assert!(player.steam.played().is_empty());
}

#[tokio::test(start_paused = true)]
async fn a_new_first_choice_takes_over_within_moments() {
    let mut player = Player::new();
    player.steam.add_game(1, 5.0, 5, Some(HOUR));
    player.steam.add_game(2, 5.0, 5, Some(HOUR));
    player.starts_farming();
    assert_eq!(
        player.reads(playing).await,
        FarmingCards {
            game: "Game 1".into(),
            cards_left: 5
        }
    );

    player.wants(Preferences {
        priority_games: vec![AppId(2)],
        ..Default::default()
    });

    assert_eq!(
        player.reads(|e| matches!(e, MovedOn { .. })).await,
        MovedOn {
            game: "Game 1".into(),
            outranked: true
        }
    );
    assert_eq!(
        player.reads(playing).await,
        FarmingCards {
            game: "Game 2".into(),
            cards_left: 5
        }
    );
}

#[tokio::test(start_paused = true)]
async fn nothing_to_farm_says_why() {
    let mut player = Player::new();
    player.steam.add_game(620, 5.0, 0, None);
    player.starts_farming();

    let idle = player.sees(Status::Idle).await;
    assert_eq!(idle.nothing_to_farm, Some(NothingToFarm::AllDropped));
    assert!(idle.next_look.is_some(), "it says when it'll look again");
    assert!(player.steam.played().is_empty());
}

#[tokio::test(start_paused = true)]
async fn sale_event_badges_are_never_played() {
    let mut player = Player::new();
    player.steam.add_game(2_861_720, 0.0, 3, Some(HOUR));
    player.starts_farming();

    let idle = player.sees(Status::Idle).await;
    assert_eq!(idle.nothing_to_farm, Some(NothingToFarm::NotDropping));
    assert!(player.steam.played().is_empty());
}
