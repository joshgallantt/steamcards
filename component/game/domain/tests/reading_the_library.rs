//! Acceptance tier: the library as the user meets it.

use std::sync::{Arc, atomic::Ordering};

use game::{
    DefaultReadLibraryUseCase, GameError, ReadLibraryUseCase,
    test_support::{InMemoryGameRepository, game},
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
    let repo = Arc::new(InMemoryGameRepository::with(vec![
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
    let repo = Arc::new(InMemoryGameRepository::with(Vec::new()));
    repo.down.store(true, Ordering::Relaxed);

    let read = DefaultReadLibraryUseCase::new(repo).call().await.unwrap();

    assert_eq!(
        read,
        Err(GameError::Unavailable(
            "steamcommunity.com didn't answer".into()
        ))
    );
}
