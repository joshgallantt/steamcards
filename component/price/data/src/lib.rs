//! The price domain's contract, satisfied by Steam and the disk. The
//! repository looks prices up through a client, and keeps them through a
//! store. The client is Steam's: a set's cards from the market's search and
//! a card's order book, read here (see `market`) and asked for through one
//! queue at the market's pace, and the wallet from the CM connection. The store keeps the prices in a file
//! of their own, and the basis and Steam's pause in the config file. Imports
//! `price` because the contract is declared there; `price` imports nothing
//! back.

mod default_price_repository;
mod dto;
mod market;
mod market_client;
mod price_store;

pub use default_price_repository::DefaultPriceRepository;
pub use market::MarketPace;
pub use market_client::{MarketClient, SteamMarketClient};
pub use price_store::{FilePriceStore, PriceStore};
