//! Acceptance tier: which cards new items are, as the user meets it when
//! Steam can't say. What Steam says of each item (each copy on its own,
//! in the order asked, only cards) is Steam's own rule, tested against a
//! stand-in for it in card-data and steam-api.

use std::sync::{Arc, atomic::Ordering};

use card::{
    AssetId, CardError, DefaultIdentifyCardsUseCase, IdentifyCardsUseCase,
    test_support::InMemoryCardRepository,
};

#[tokio::test]
async fn cards_that_cant_be_described_say_why() {
    let repo = Arc::new(InMemoryCardRepository::default());
    repo.down.store(true, Ordering::Relaxed);

    let described = DefaultIdentifyCardsUseCase::new(repo)
        .call(vec![AssetId(31_001)])
        .await
        .unwrap();

    assert_eq!(
        described,
        Err(CardError::Unavailable("Steam didn't answer in time".into()))
    );
}
