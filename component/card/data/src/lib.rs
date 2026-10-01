//! The card domain's contracts, satisfied by Steam and the disk. The cards
//! are read through a client, Steam's: card pages (foils' too) from
//! steamcommunity.com, and the inventory's items over the CM connection.
//! Prices are looked up through a client of their own and kept through a
//! store. That client is Steam's too: a set's cards from the market's
//! search, read here (see `market`) and asked for through one queue at the
//! market's pace. The store keeps the prices in a file of their own, and
//! Steam's pause in the config file. Imports `card` because the contracts
//! are declared there; `card` imports nothing back.

mod card_client;
mod default_card_price_repository;
mod default_card_repository;
mod dto;
mod market;
mod market_client;
mod price_store;

pub use card_client::{CardClient, SteamCardClient};
pub use default_card_price_repository::DefaultCardPriceRepository;
pub use default_card_repository::DefaultCardRepository;
pub use market::MarketPace;
pub use market_client::{MarketClient, SteamMarketClient};
pub use price_store::{FilePriceStore, PriceStore};
