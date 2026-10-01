use serde::{Deserialize, Serialize};

use crate::dto::MarketPauseDto;

/// Steam's pause on market requests, as kept.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(crate) struct MarketDto {
    /// Steam's pause on market requests, when there is one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) pause: Option<MarketPauseDto>,
}
