//! Unit tier: reading the library, the use case over a fake repository. The
//! acceptance tier, through the steam-library component and stand-ins for
//! Steam and its site, is in steam-library-di.

use std::sync::{Arc, atomic::Ordering};

use steam_library::{
    DefaultReadLibraryUseCase, ReadLibraryUseCase, SteamLibraryError,
    test_support::{FakeSteamLibraryRepository, game},
};

#[tokio::test]
async fn games_with_drops_left_come_first_most_played_first() {
    let mut half_life = game(220, 30.0, 3, 0);
    half_life.name = "Half-Life 2".into();
    let mut portal = game(620, 5.2, 1, 3);
    portal.name = "Portal 2".into();
    let mut stardew = game(413150, 0.0, 0, 4);
    stardew.name = "Stardew Valley".into();
    let mut hades = game(1145360, 12.5, 3, 1);
    hades.name = "Hades".into();
    let repo = Arc::new(FakeSteamLibraryRepository::with(vec![
        half_life, portal, stardew, hades,
    ]));

    let library = DefaultReadLibraryUseCase::new(repo)
        .call()
        .await
        .unwrap()
        .unwrap();

    let names: Vec<&str> = library.games().iter().map(|g| g.name.as_str()).collect();
    assert_eq!(
        names,
        ["Hades", "Portal 2", "Stardew Valley", "Half-Life 2"]
    );
    assert_eq!(library.drops_left(), 8);
}

#[tokio::test]
async fn a_library_that_cant_be_read_says_why() {
    let repo = Arc::new(FakeSteamLibraryRepository::with(Vec::new()));
    repo.down.store(true, Ordering::Relaxed);

    let read = DefaultReadLibraryUseCase::new(repo).call().await.unwrap();

    assert_eq!(
        read,
        Err(SteamLibraryError::Unavailable(
            "steamcommunity.com didn't answer".into()
        ))
    );
}
