//! Acceptance tier: which cards new items are, as the user meets them. The
//! same card can drop more than once, and each copy is its own drop.

use std::sync::{Arc, atomic::Ordering};

use library::{
    LibraryError, describe_cards,
    test_support::{InMemoryLibraryRepository, card_asset},
};

#[tokio::test]
async fn a_card_that_drops_twice_is_two_copies() {
    // Heavy Rain has dropped Madison twice: once before, and again at 17:05.
    let first = card_asset(31_001, 960_910, "Madison");
    let second = card_asset(31_002, 960_910, "Madison");
    let repo =
        Arc::new(InMemoryLibraryRepository::default().holding(vec![first.clone(), second.clone()]));

    let cards = describe_cards(repo)(vec![31_001, 31_002])
        .await
        .unwrap()
        .unwrap();

    assert_eq!(cards, [first, second], "one card, two copies");
}

#[tokio::test]
async fn only_the_cards_among_new_items_are_described() {
    let mut zagreus = card_asset(45_001, 1_145_360, "Zagreus");
    zagreus.foil = true;
    let scott = card_asset(45_002, 960_910, "Scott");
    let repo = Arc::new(
        InMemoryLibraryRepository::default().holding(vec![zagreus.clone(), scott.clone()]),
    );

    // 45_003 is an emoticon, and 99 isn't in the inventory.
    let cards = describe_cards(repo)(vec![45_002, 45_003, 45_001, 99])
        .await
        .unwrap()
        .unwrap();

    assert_eq!(cards, [scott, zagreus], "in the order asked");
}

#[tokio::test]
async fn cards_that_cant_be_described_say_why() {
    let repo = Arc::new(InMemoryLibraryRepository::default());
    repo.down.store(true, Ordering::Relaxed);

    let described = describe_cards(repo)(vec![31_001]).await.unwrap();

    assert_eq!(
        described,
        Err(LibraryError::Unavailable(
            "Steam didn't answer in time".into()
        ))
    );
}
