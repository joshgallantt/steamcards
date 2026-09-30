//! Acceptance tier: a game's cards as the user meets them.

use std::sync::Arc;

use card::{
    CardSet, look_at_cards, look_at_foils,
    test_support::{InMemoryCardRepository, card},
};
use game::test_support::game;

#[tokio::test]
async fn a_games_cards_are_looked_at_on_its_own_page() {
    let repo = Arc::new(InMemoryCardRepository::with(vec![game(620, 5.2, 1, 3)]));
    let set = CardSet::new(vec![card("Atlas", 1), card("P-Body", 0)]);
    repo.sets.lock().unwrap().insert(620, set.clone());
    let look = look_at_cards(repo.clone());

    let looked = look(620).await.unwrap().unwrap();

    assert_eq!(looked.game.drops.remaining, 3);
    assert_eq!(looked.set, set);
    assert!(look(1).await.unwrap().is_err(), "no such game");
}

#[tokio::test]
async fn a_game_with_no_foils_held_has_none_counted() {
    let repo = Arc::new(InMemoryCardRepository::with(vec![game(620, 5.2, 1, 3)]));

    let foils = look_at_foils(repo)(620).await.unwrap().unwrap();

    assert!(foils.is_empty());
}
