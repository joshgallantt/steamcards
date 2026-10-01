use game::{AppId, CardDrops, Game};

/// A game's row on the badge pages, as read.
#[derive(Debug, Clone)]
pub(crate) struct BadgeDto {
    pub(crate) app_id: u32,
    pub(crate) name: String,
    /// Hours on record.
    pub(crate) hours: f64,
    /// Card drops still to come from playing it.
    pub(crate) cards_left: u32,
    /// Card drops so far.
    pub(crate) cards_received: u32,
    /// The badge's level: 0 until one is crafted.
    pub(crate) badge_level: u8,
    /// The badge pages may be wrong about it: its own card page is asked.
    pub(crate) unsure: bool,
}

impl BadgeDto {
    pub(crate) fn into_domain(self) -> Game {
        Game {
            app_id: AppId(self.app_id),
            name: self.name,
            hours: self.hours,
            drops: CardDrops {
                received: self.cards_received,
                remaining: self.cards_left,
            },
            badge_level: self.badge_level,
        }
    }
}
