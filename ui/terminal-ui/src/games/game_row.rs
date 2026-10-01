use steam_library::{AppId, CardDrops};

/// One line in a list of games to pick from.
#[derive(Debug, Clone, PartialEq)]
pub struct GameRow {
    pub app_id: AppId,
    pub name: String,
    /// 1-based position among the priority games.
    pub rank: Option<usize>,
    pub hours: f64,
    pub drops: CardDrops,
}
