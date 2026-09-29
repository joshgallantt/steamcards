use std::fmt;

/// The games on the account that have trading cards: what farming works
/// through. A game appears once.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SteamLibrary {
    games: Vec<Game>,
}

impl SteamLibrary {
    /// The library of `games`, in their order; a game listed twice is kept
    /// once, as first listed.
    pub fn new(games: Vec<Game>) -> Self {
        let mut library = Self::default();
        for game in games {
            if library.game(game.app_id).is_none() {
                library.games.push(game);
            }
        }
        library
    }

    pub fn games(&self) -> &[Game] {
        &self.games
    }

    pub fn game(&self, app_id: u32) -> Option<&Game> {
        self.games.iter().find(|g| g.app_id == app_id)
    }

    /// The games that still have card drops to come.
    pub fn with_drops_left(&self) -> impl Iterator<Item = &Game> {
        self.games.iter().filter(|g| g.has_drops_left())
    }

    /// Card drops still to come, across the library.
    pub fn drops_left(&self) -> u32 {
        self.games.iter().map(|g| g.drops.remaining).sum()
    }

    /// Puts a newer look at a game in place of the older one, or adds it.
    pub fn update(&mut self, game: Game) {
        match self.games.iter_mut().find(|g| g.app_id == game.app_id) {
            Some(old) => *old = game,
            None => self.games.push(game),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.games.is_empty()
    }
}

/// A game on the account that has trading cards. Its identity is its Steam
/// app ID.
#[derive(Debug, Clone, PartialEq)]
pub struct Game {
    pub app_id: u32,
    pub name: String,
    /// Hours on record, as Steam counts them.
    pub hours: f64,
    /// Card drops so far, and still to come from playing it.
    pub drops: CardDrops,
    /// The level of the badge crafted from its cards: 0 until one is.
    pub badge_level: u8,
    /// Its set of trading cards, and how many of each the account has.
    /// Empty until the game's own card page has been read.
    pub cards: Vec<Card>,
}

impl Game {
    pub fn has_drops_left(&self) -> bool {
        self.drops.remaining > 0
    }

    /// How many cards of its set the account has at least one of.
    pub fn cards_collected(&self) -> usize {
        self.cards.iter().filter(|c| c.owned > 0).count()
    }

    /// Whether the account has every card of the set, so a badge can be
    /// crafted. `false` while the set isn't known.
    pub fn has_full_set(&self) -> bool {
        !self.cards.is_empty() && self.cards.iter().all(|c| c.owned > 0)
    }
}

/// A game's card drops. Playing a game drops about half its set; which cards
/// drop is up to Steam.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CardDrops {
    /// Cards dropped so far.
    pub received: u32,
    /// Cards still to drop.
    pub remaining: u32,
}

impl CardDrops {
    /// Every card playing the game drops, dropped or not.
    pub fn total(&self) -> u32 {
        self.received + self.remaining
    }
}

/// A trading card from a game's set. Its identity is its name, within the
/// set.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Card {
    pub name: String,
    /// How many the account has.
    pub owned: u32,
}

/// Why the library couldn't be read, in the user's terms. Steam's reason is
/// theirs to read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LibraryError {
    Unavailable(String),
}

impl fmt::Display for LibraryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LibraryError::Unavailable(why) => write!(f, "{why}"),
        }
    }
}

impl std::error::Error for LibraryError {}

#[cfg(test)]
mod tests {
    use super::*;

    fn game(app_id: u32, received: u32, remaining: u32) -> Game {
        Game {
            app_id,
            name: format!("Game {app_id}"),
            hours: 0.0,
            drops: CardDrops {
                received,
                remaining,
            },
            badge_level: 0,
            cards: Vec::new(),
        }
    }

    fn card(name: &str, owned: u32) -> Card {
        Card {
            name: name.into(),
            owned,
        }
    }

    #[test]
    fn a_library_counts_the_drops_still_to_come() {
        let library = SteamLibrary::new(vec![game(620, 1, 3), game(220, 3, 0), game(440, 0, 2)]);
        assert_eq!(library.drops_left(), 5);
        let left: Vec<u32> = library.with_drops_left().map(|g| g.app_id).collect();
        assert_eq!(left, [620, 440]);
    }

    #[test]
    fn a_game_is_in_the_library_once() {
        let library = SteamLibrary::new(vec![game(620, 1, 3), game(620, 0, 0)]);
        assert_eq!(library.games().len(), 1);
        assert_eq!(
            library.game(620).unwrap().drops.remaining,
            3,
            "the first one"
        );
    }

    #[test]
    fn a_newer_look_at_a_game_replaces_the_old() {
        let mut library = SteamLibrary::new(vec![game(620, 1, 3)]);
        library.update(game(620, 2, 2));
        library.update(game(730, 0, 1));
        assert_eq!(
            library.game(620).unwrap().drops,
            CardDrops {
                received: 2,
                remaining: 2,
            }
        );
        assert_eq!(library.games().len(), 2);
    }

    #[test]
    fn a_game_knows_its_set() {
        let mut portal = game(620, 2, 2);
        assert!(!portal.has_full_set(), "the set isn't known yet");
        portal.cards = vec![card("Atlas", 2), card("P-Body", 0), card("Wheatley", 1)];
        assert_eq!(portal.cards_collected(), 2);
        assert!(!portal.has_full_set());
        portal.cards[1].owned = 1;
        assert!(portal.has_full_set());
        assert_eq!(portal.drops.total(), 4);
    }
}
