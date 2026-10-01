use std::collections::BTreeMap;

use game::AppId;

use crate::CardSet;

/// The card sets of the games looked at so far, by app ID: each as last
/// read, with the copies counted in since.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CardSets {
    sets: BTreeMap<AppId, CardSet>,
}

impl CardSets {
    /// A game's set, once it has been read.
    pub fn set(&self, app_id: AppId) -> Option<&CardSet> {
        self.sets.get(&app_id)
    }

    /// Puts a newer read of a game's set in place of the older one, or adds
    /// it.
    pub fn update(&mut self, app_id: AppId, set: CardSet) {
        self.sets.insert(app_id, set);
    }
}
