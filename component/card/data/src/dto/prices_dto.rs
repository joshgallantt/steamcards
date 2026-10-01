use serde::{Deserialize, Serialize};

use crate::dto::PriceSetDto;

/// The prices file: every set kept.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(crate) struct PricesDto {
    #[serde(default)]
    pub(crate) sets: Vec<PriceSetDto>,
}
