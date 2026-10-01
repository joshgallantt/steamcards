//! Acceptance tier: the library as the user meets it, through the game
//! component as steamcards wires it, against stand-ins for Steam and its
//! site.

mod support;

use steam_library::{AppId, SteamLibraryError};
use support::Player;
use wiremock::{Mock, ResponseTemplate, matchers::method};

#[tokio::test]
async fn games_with_drops_left_come_first_most_played_first() {
    let player = Player::new("order").await;
    player.has_badges().await;

    let library = player.reads_the_library().await.unwrap();

    let games = library.games();
    let left = games.iter().take_while(|g| g.has_drops_left()).count();
    assert_eq!(left, 5, "every game with drops left");
    assert!(
        games[left..].iter().all(|g| !g.has_drops_left()),
        "then the finished ones"
    );
    assert!(
        games[..left].windows(2).all(|w| w[0].hours >= w[1].hours),
        "most played first"
    );
    assert_eq!(
        games[0].app_id,
        AppId(730),
        "350 hours, as its own card page says"
    );
}

#[tokio::test]
async fn a_library_that_cant_be_read_says_why() {
    let player = Player::new("unread").await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&player.site)
        .await;

    let read = player.reads_the_library().await;

    assert!(
        matches!(&read, Err(SteamLibraryError::Unavailable(why)) if why.contains("steamcommunity.com")),
        "{read:?}"
    );
}
