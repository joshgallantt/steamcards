//! Unit tier: playing games, the game's use cases over a fake repository.
//! The farmer plays through these for hours on end, in farming's own
//! tests; the data layer's are against a stand-in for Steam.

use std::sync::{Arc, atomic::Ordering};

use game::{
    AppId, DefaultObservePlayingUseCase, DefaultPlayGamesUseCase, DefaultStandByUseCase,
    DefaultStopPlayingUseCase, GameError, ObservePlayingUseCase, PlayGamesUseCase, Playing,
    PlayingSignal, StandByUseCase, StopPlayingUseCase, test_support::FakePlayingRepository,
};

#[tokio::test]
async fn games_play_here_until_another_device_plays() {
    let steam = Arc::new(FakePlayingRepository::default());
    let play = DefaultPlayGamesUseCase::new(steam.clone());

    let here = play.call(&[AppId(620)], true).await;
    *steam.elsewhere.lock().unwrap() = Some(Some(AppId(730)));
    let elsewhere = play.call(&[AppId(620)], true).await;

    assert_eq!(here, Ok(Playing::Here));
    assert_eq!(elsewhere, Ok(Playing::Elsewhere(Some(AppId(730)))));
    assert_eq!(
        *steam.plays.lock().unwrap(),
        [(vec![AppId(620)], true)],
        "nothing is played while another device plays"
    );
}

#[tokio::test]
async fn standing_by_signs_on_and_plays_nothing() {
    let steam = Arc::new(FakePlayingRepository::default());
    *steam.elsewhere.lock().unwrap() = Some(None);

    let playing = DefaultStandByUseCase::new(steam.clone()).call().await;

    assert_eq!(playing, Ok(Playing::Elsewhere(None)), "its game unsaid");
    assert!(steam.signed_on.load(Ordering::Relaxed));
    assert!(steam.plays.lock().unwrap().is_empty());
}

#[tokio::test]
async fn playing_when_steam_cant_be_reached_says_why() {
    let steam = Arc::new(FakePlayingRepository::default());
    steam.down.store(true, Ordering::Relaxed);

    let played = DefaultPlayGamesUseCase::new(steam.clone())
        .call(&[AppId(620)], false)
        .await;
    let stood_by = DefaultStandByUseCase::new(steam).call().await;

    let unavailable = Err(GameError::Unavailable("couldn't reach Steam".into()));
    assert_eq!(played, unavailable);
    assert_eq!(stood_by, unavailable);
}

#[tokio::test]
async fn what_steam_says_of_playing_is_heard_in_order() {
    let steam = Arc::new(FakePlayingRepository::default());
    let observe = DefaultObservePlayingUseCase::new(steam.clone());

    steam.says(PlayingSignal::Blocked(Some(AppId(730))));
    steam.says(PlayingSignal::Unblocked);

    assert_eq!(
        observe.call().await,
        PlayingSignal::Blocked(Some(AppId(730)))
    );
    assert_eq!(observe.call().await, PlayingSignal::Unblocked);
}

#[tokio::test]
async fn stopping_signs_off() {
    let steam = Arc::new(FakePlayingRepository::default());
    DefaultPlayGamesUseCase::new(steam.clone())
        .call(&[AppId(620)], false)
        .await
        .unwrap();

    DefaultStopPlayingUseCase::new(steam.clone()).call().await;

    assert!(!steam.signed_on.load(Ordering::Relaxed));
    assert_eq!(steam.stops.load(Ordering::Relaxed), 1);
}
