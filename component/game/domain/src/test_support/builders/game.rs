use crate::{AppId, CardDrops, Game};

/// A game with `received` and `remaining` card drops and `hours` played,
/// named "Game <app ID>" unless named after.
pub fn game(app_id: u32, hours: f64, received: u32, remaining: u32) -> Game {
    Game {
        app_id: AppId(app_id),
        name: format!("Game {app_id}"),
        hours,
        drops: CardDrops {
            received,
            remaining,
        },
        badge_level: 0,
    }
}
