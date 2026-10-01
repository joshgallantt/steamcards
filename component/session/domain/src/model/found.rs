use card::CardSet;
use steam_library::AppId;

use crate::Looked;

/// Drops that one look at a game, or one read of the library, found.
#[derive(Debug)]
pub struct Found {
    pub app_id: AppId,
    /// Where they are in the session's drops.
    pub(crate) drops: Vec<usize>,
    /// The set as the farmer knew it before them, every earlier drop counted
    /// in; empty when it didn't know it so.
    pub(crate) before: CardSet,
    /// The set a look at the game found, with them in it. The badge pages
    /// show no sets, so a read has none until the game is looked at.
    pub(crate) after: Option<CardSet>,
}

impl Found {
    /// How many cards dropped.
    pub fn count(&self) -> usize {
        self.drops.len()
    }

    /// Found by a read: no set says which cards they were until the game is
    /// looked at.
    pub fn needs_look(&self) -> bool {
        self.after.is_none()
    }

    /// Takes in a look at the game since: the set it read, and any drops it
    /// found meanwhile, which are told with these.
    pub fn join(&mut self, looked: Looked) {
        self.after = looked.set;
        if let Some(more) = looked.found {
            self.drops.extend(more.drops);
        }
    }
}
