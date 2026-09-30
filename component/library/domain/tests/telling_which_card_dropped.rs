//! Acceptance tier: which cards new items are, as the user meets it when
//! Steam can't say. What Steam says of each item (each copy on its own,
//! in the order asked, only cards) is Steam's own rule, tested against a
//! stand-in for it in library-data and steam-api.

use std::sync::{Arc, atomic::Ordering};

use library::{LibraryError, describe_cards, test_support::InMemoryLibraryRepository};

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
