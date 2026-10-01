//! Unit tier: a game's cards, the use cases over a fake repository. The
//! acceptance tier, through the card component and stand-ins for Steam and
//! its site, is in card-di.

use std::sync::Arc;

use card::{
    CardSet, DefaultLookAtCardsUseCase, DefaultLookAtFoilsUseCase, LookAtCardsUseCase,
    LookAtFoilsUseCase,
    test_support::{FakeCardRepository, card},
};
use game::{AppId, test_support::game};

#[tokio::test]
async fn a_games_cards_are_looked_at_on_its_own_page() {
    let repo = Arc::new(FakeCardRepository::with(vec![game(620, 5.2, 1, 3)]));
    let set = CardSet::new(vec![card("Atlas", 1), card("P-Body", 0)]);
    repo.sets.lock().unwrap().insert(AppId(620), set.clone());
    let look = DefaultLookAtCardsUseCase::new(repo.clone());

    let looked = look.call(AppId(620)).await.unwrap().unwrap();

    assert_eq!(looked.game.drops.remaining, 3);
    assert_eq!(looked.set, set);
    assert!(look.call(AppId(1)).await.unwrap().is_err(), "no such game");
}

#[tokio::test]
async fn a_game_with_no_foils_held_has_none_counted() {
    let repo = Arc::new(FakeCardRepository::with(vec![game(620, 5.2, 1, 3)]));

    let foils = DefaultLookAtFoilsUseCase::new(repo)
        .call(AppId(620))
        .await
        .unwrap()
        .unwrap();

    assert!(foils.is_empty());
}
