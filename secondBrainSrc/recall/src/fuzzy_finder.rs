use activity_tracker_common::{
    ActivitySummary,
    db::{GeneralDbClient, SummaryStore}
};
use std::error::Error;

#[derive(Clone)]
pub struct FuzzyFinder {
    pub db_client: GeneralDbClient,
}

impl FuzzyFinder {
    pub fn new(db_client: GeneralDbClient) -> Self {
        Self { db_client }
    }

    pub async fn search(&self, query: &str) -> Result<Vec<ActivitySummary>, Box<dyn Error + Send + Sync>> {
        // @todo this would implement fuzzy finding logic
        match self.db_client.search_summaries(query).await {
            Ok(results) => Ok(results),
            Err(e) => Err(Box::new(std::io::Error::new(std::io::ErrorKind::Other, e.to_string())) as Box<dyn Error + Send + Sync>),
        }
    }
}
