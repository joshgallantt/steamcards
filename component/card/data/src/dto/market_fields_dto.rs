use serde::{Deserialize, Serialize};

use crate::dto::MarketDto;

/// The market's field in the config file.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(crate) struct MarketFieldsDto {
    #[serde(default)]
    pub(crate) market: MarketDto,
}
