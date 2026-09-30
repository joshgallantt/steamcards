use serde::{Deserialize, Serialize};

use crate::dto::MarketPauseDto;

/// The market's settings, and Steam's pause on market requests, as kept.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(crate) struct MarketDto {
    /// The value basis: "list", "net" or "instant". Empty for the default.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub(crate) basis: String,
    /// Steam's pause on market requests, when there is one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) pause: Option<MarketPauseDto>,
}
