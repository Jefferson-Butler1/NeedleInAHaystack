use activity_tracker_common::{GeneralDbClient, SummaryStore, ActivitySummary};
use std::error::Error;

#[derive(Clone)]
pub struct FuzzyFinder {
    pub db_client: GeneralDbClient
}

impl FuzzyFinder {
    pub fn new(db_client: GeneralDbClient) -> Self {
        Self { db_client }
    }

    pub async fn search(&self, query: &str) -> Result<Vec<ActivitySummary>, Box<dyn Error>> {
        // @todo this would implement fuzzy finding logic
        self.db_client.search_summaries(query).await
    }
}
