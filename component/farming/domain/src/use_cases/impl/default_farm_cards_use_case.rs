use std::sync::Arc;

use tokio::{sync::mpsc, task::JoinHandle};
use tokio_util::sync::CancellationToken;

use crate::{FarmCardsUseCase, FarmingDependencies, FarmingUpdate, farmer::Farmer};

pub struct DefaultFarmCardsUseCase {
    farmer: Arc<Farmer>,
}

impl DefaultFarmCardsUseCase {
    pub fn new(dependencies: FarmingDependencies) -> Self {
        Self {
            farmer: Arc::new(Farmer::new(dependencies)),
        }
    }
}

impl FarmCardsUseCase for DefaultFarmCardsUseCase {
    fn call(
        &self,
        token: CancellationToken,
        updates: mpsc::Sender<FarmingUpdate>,
    ) -> JoinHandle<()> {
        let farmer = Arc::clone(&self.farmer);
        tokio::spawn(async move { farmer.farm(token, updates).await })
    }
}
