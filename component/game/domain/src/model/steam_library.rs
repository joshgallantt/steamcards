use crate::Game;

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

    /// Card drops received so far, across the library.
    pub fn drops_received(&self) -> u32 {
        self.games.iter().map(|g| g.drops.received).sum()
    }

    /// Every card drop playing the library's games gives, dropped or not.
    pub fn drops_total(&self) -> u32 {
        self.games.iter().map(|g| g.drops.total()).sum()
    }

    /// How many games have had every drop they give. A game that gives none
    /// isn't counted: there was nothing to farm.
    pub fn games_done(&self) -> usize {
        self.games
            .iter()
            .filter(|g| g.drops.total() > 0 && !g.has_drops_left())
            .count()
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::CardDrops;

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
    fn a_library_counts_its_progress() {
        let library = SteamLibrary::new(vec![
            game(620, 1, 3),
            game(220, 3, 0),
            game(440, 0, 2),
            game(730, 0, 0),
        ]);
        assert_eq!(library.drops_received(), 4);
        assert_eq!(library.drops_total(), 9);
        assert_eq!(library.drops_left(), 5);
        assert_eq!(
            library.games_done(),
            1,
            "Half-Life 2 has had every drop; 730 never had one to give"
        );
        assert_eq!(SteamLibrary::default().games_done(), 0);
    }
}
