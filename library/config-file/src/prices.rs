//! The market's prices, in a JSON file of their own beside the config file,
//! so a restart doesn't have to look every game up again. It's a cache: a
//! file that's missing, or doesn't read, is an empty one, and the next save
//! writes it afresh. The stored shape is these types, as they are.

use std::{fs, path::PathBuf, sync::Mutex};

use serde::{Deserialize, Serialize};

use crate::write_whole;

/// A game's set of cards, priced, as stored.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoredSet {
    pub app_id: u32,
    /// When it was looked up, in seconds since the epoch.
    pub fetched_at: i64,
    /// When its last lookup failed, it's tried again then.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retry_at: Option<i64>,
    #[serde(default)]
    pub normal: Vec<StoredCard>,
    #[serde(default)]
    pub foil: Vec<StoredCard>,
}

/// A card of a set, and its price, as stored.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoredCard {
    pub name: String,
    pub market_hash_name: String,
    pub price: StoredPrice,
}

/// A price, as stored. `state` is "known", "no market", "not marketable",
/// "failed" or "pending"; the rest is what a known price, or a failed
/// lookup, has.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoredPrice {
    pub state: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ask: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bid: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ask_depth: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bid_depth: Option<u32>,
    /// Steam's `ECurrency` id for the amounts.
    #[serde(default)]
    pub currency: u32,
    /// "search" or "order book".
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub source: String,
    /// In seconds since the epoch.
    #[serde(default)]
    pub fetched_at: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retry_at: Option<i64>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct CacheDto {
    #[serde(default)]
    sets: Vec<StoredSet>,
}

/// The file the market's prices are kept in.
pub struct PriceCache {
    path: PathBuf,
    sets: Mutex<Vec<StoredSet>>,
}

impl PriceCache {
    /// Reads the file at `path`: empty when there's none, or it doesn't
    /// read.
    pub fn open(path: PathBuf) -> Self {
        let sets = fs::read(&path)
            .ok()
            .and_then(|data| serde_json::from_slice::<CacheDto>(&data).ok())
            .unwrap_or_default()
            .sets;
        Self {
            path,
            sets: Mutex::new(sets),
        }
    }

    pub fn sets(&self) -> Vec<StoredSet> {
        self.sets.lock().unwrap().clone()
    }

    /// Keeps `sets` in place of what was there. Writes first, then keeps: a
    /// failed write leaves what was there.
    pub fn save_sets(&self, sets: Vec<StoredSet>) -> anyhow::Result<()> {
        let mut kept = self.sets.lock().unwrap();
        let dto = CacheDto { sets };
        write_whole(&self.path, &serde_json::to_vec(&dto)?)?;
        *kept = dto.sets;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("steamcards-prices-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        dir.join("prices.json")
    }

    fn madison() -> StoredSet {
        StoredSet {
            app_id: 960_910,
            fetched_at: 1_790_673_240,
            retry_at: None,
            normal: vec![StoredCard {
                name: "Madison".into(),
                market_hash_name: "960910-Madison".into(),
                price: StoredPrice {
                    state: "known".into(),
                    ask: Some(5),
                    ask_depth: Some(1_204),
                    currency: 2,
                    source: "search".into(),
                    fetched_at: 1_790_673_240,
                    ..Default::default()
                },
            }],
            foil: Vec::new(),
        }
    }

    #[test]
    fn prices_saved_are_read_back_after_a_restart() {
        let path = temp("roundtrip");
        let cache = PriceCache::open(path.clone());
        assert!(cache.sets().is_empty(), "no file yet");

        cache.save_sets(vec![madison()]).unwrap();

        assert_eq!(PriceCache::open(path).sets(), [madison()]);
    }

    #[test]
    fn a_cache_that_doesnt_read_is_an_empty_one() {
        let path = temp("garbled");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, "{\"sets\": [{\"app_").unwrap();

        let cache = PriceCache::open(path.clone());

        assert!(cache.sets().is_empty());
        cache.save_sets(vec![madison()]).unwrap();
        assert_eq!(PriceCache::open(path).sets().len(), 1, "written afresh");
    }

    #[test]
    fn a_failed_write_keeps_what_was_there() {
        // A directory where the file should be makes the rename fail.
        let path = temp("unwritable");
        fs::create_dir_all(&path).unwrap();
        let cache = PriceCache::open(path);
        assert!(cache.save_sets(vec![madison()]).is_err());
        assert!(cache.sets().is_empty());
    }
}
