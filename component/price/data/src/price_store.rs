use std::{path::PathBuf, sync::Arc};

use config_file::{ConfigFile, JsonFile};
use price::{Basis, MarketPause, PriceBook, PriceSettings, SetPrices};

use crate::dto::{MarketFieldsDto, MarketPauseDto, PriceSetDto, PricesDto};

/// Where the prices are kept, and the market's settings.
pub trait PriceStore: Send + Sync {
    /// The sets kept, each as it was. One that doesn't read is left out, so
    /// it's looked up afresh.
    fn sets(&self) -> Vec<SetPrices>;

    /// Keeps the book's sets in place of those kept; its offers aren't kept.
    /// Errs when they couldn't be.
    fn save_sets(&self, book: &PriceBook) -> anyhow::Result<()>;

    fn settings(&self) -> PriceSettings;

    /// Errs when they couldn't be kept, so nothing reports a change that
    /// didn't happen.
    fn save_settings(&self, settings: PriceSettings) -> anyhow::Result<()>;

    /// Steam's pause on market requests, as kept: to the second.
    fn pause(&self) -> Option<MarketPause>;

    /// Keeps Steam's pause when it has changed, to the second; `None` once
    /// there's none. Errs when it couldn't be kept.
    fn save_pause(&self, pause: Option<MarketPause>) -> anyhow::Result<()>;
}

/// The prices in a file of their own, and the market's settings and Steam's
/// pause in the config file, beside its other fields.
pub struct FilePriceStore {
    config: Arc<ConfigFile>,
    prices: JsonFile<PricesDto>,
}

impl FilePriceStore {
    /// Keeps the prices in the file at `prices`.
    pub fn open(config: Arc<ConfigFile>, prices: PathBuf) -> Self {
        Self {
            config,
            prices: JsonFile::open(prices),
        }
    }

    fn market(&self) -> MarketFieldsDto {
        self.config.read().unwrap_or_default()
    }
}

impl PriceStore for FilePriceStore {
    fn sets(&self) -> Vec<SetPrices> {
        self.prices
            .get()
            .sets
            .into_iter()
            .filter_map(PriceSetDto::into_domain)
            .collect()
    }

    fn save_sets(&self, book: &PriceBook) -> anyhow::Result<()> {
        self.prices.save(PricesDto {
            sets: book.sets.values().map(PriceSetDto::new).collect(),
        })
    }

    fn settings(&self) -> PriceSettings {
        let basis = match self.market().market.basis.as_str() {
            "net" => Basis::Net,
            "instant" => Basis::Instant,
            _ => Basis::List,
        };
        PriceSettings { basis }
    }

    fn save_settings(&self, settings: PriceSettings) -> anyhow::Result<()> {
        let mut fields = self.market();
        fields.market.basis = match settings.basis {
            Basis::List => "list",
            Basis::Net => "net",
            Basis::Instant => "instant",
        }
        .to_owned();
        self.config.write(&fields)
    }

    fn pause(&self) -> Option<MarketPause> {
        self.market().market.pause.map(MarketPauseDto::into_domain)
    }

    fn save_pause(&self, pause: Option<MarketPause>) -> anyhow::Result<()> {
        let mut fields = self.market();
        let pause = pause.map(MarketPauseDto::new);
        if fields.market.pause == pause {
            return Ok(());
        }
        fields.market.pause = pause;
        self.config.write(&fields)
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    #[test]
    fn a_config_file_from_before_the_market_reads_with_its_defaults() {
        let dir = std::env::temp_dir().join(format!(
            "steamcards-price-store-before-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let config = dir.join("config.json");
        fs::write(&config, r#"{"priority_games":[620]}"#).unwrap();

        let store = FilePriceStore::open(
            Arc::new(ConfigFile::open(config).unwrap()),
            dir.join("prices.json"),
        );

        assert_eq!(store.settings().basis, Basis::List);
        assert_eq!(store.pause(), None);
        assert!(store.sets().is_empty(), "no prices kept yet");
    }
}
