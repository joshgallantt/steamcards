//! Unit tier: telling which card dropped, the use cases over a fake
//! repository: hearing of new items as Steam announces them, and which
//! cards they are when Steam can't say. What Steam says of each item (each
//! copy on its own, in the order asked, only cards) is Steam's own rule,
//! tested against a stand-in for it in card-data and steam-api.

use std::sync::{Arc, atomic::Ordering};

use card::{
    AssetId, CardError, DefaultIdentifyCardsUseCase, DefaultObserveNewItemsUseCase,
    IdentifyCardsUseCase, NewItem, ObserveNewItemsUseCase, test_support::FakeCardRepository,
};
use game::AppId;

#[tokio::test]
async fn new_items_are_heard_as_steam_announces_them() {
    let repo = Arc::new(FakeCardRepository::default());
    let observe = DefaultObserveNewItemsUseCase::new(repo.clone());
    let madison = NewItem {
        asset_id: AssetId(31_002),
        app_id: Some(AppId(960_910)),
        gained_at: None,
    };

    repo.announce(vec![madison]);
    repo.announce(Vec::new());

    assert_eq!(observe.call().await, [madison]);
    assert_eq!(
        observe.call().await,
        [],
        "a count alone: a card may have dropped all the same"
    );
}

#[tokio::test]
async fn cards_that_cant_be_described_say_why() {
    let repo = Arc::new(FakeCardRepository::default());
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
