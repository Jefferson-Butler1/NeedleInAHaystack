use crate::models::ActivitySummary;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use std::error::Error;

#[async_trait]
pub trait SummaryStore {
    async fn store_summary(&self, summary: &ActivitySummary) -> Result<(), Box<dyn Error>>;
    async fn get_summaries_in_timeframe(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Vec<ActivitySummary>, Box<dyn Error>>;
    async fn search_summaries(&self, query: &str) -> Result<Vec<ActivitySummary>, Box<dyn Error>>;
}

#[derive(Clone)]
pub struct GeneralDbClient {
    //@todo use SQLite for this database to store processed summaries
}

impl GeneralDbClient {
    pub fn new(connection_string: &str) -> Result<Self, Box<dyn Error>> {
        //@todo Initialize the database connection here
        Ok(Self {})
    }
}

#[async_trait]
impl SummaryStore for GeneralDbClient {
    async fn store_summary(&self, summary: &ActivitySummary) -> Result<(), Box<dyn Error>> {
        //@todo Store the summary in the database
        Ok(())
    }

    async fn get_summaries_in_timeframe(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Vec<ActivitySummary>, Box<dyn Error>> {
        //@todo Retrieve summaries within the specified timeframe
        Ok(vec![])
    }

    async fn search_summaries(&self, query: &str) -> Result<Vec<ActivitySummary>, Box<dyn Error>> {
        //@todo Perform a search for summaries matching the query
        Ok(vec![])
    }
}
