use activity_tracker_common::{
    ActivitySummary,
    db::{GeneralDbClient, SummaryStore}
};
use chrono::{DateTime, Duration, Utc};
use std::error::Error;

#[derive(Clone)]
pub struct QueryEngine {
    db_client: GeneralDbClient
}

impl QueryEngine {
    pub fn new(db_client: GeneralDbClient) -> Self {
        Self { db_client }
    }

    pub async fn process_query(&self, query: &str) -> Result<Vec<ActivitySummary>, Box<dyn Error>> {
        if let Some(time_range) = self.parse_time_query(query) {
            self.db_client.get_summaries_in_timeframe(time_range.0, time_range.1).await
        } else {
            self.db_client.search_summaries(query).await
        }
    }

    fn parse_time_query(&self, query: &str) -> Option<(DateTime<Utc>, DateTime<Utc>)> {
        let now = Utc::now();

        if query.contains("last week") {
            let end = now;
            let start = now - Duration::days(7);
            Some((start, end))
        } else if query.contains("yesterday") {
            let end = now;
            let start = now - Duration::days(1);
            Some((start, end))
        } else if query.contains("today") {
            let end = now;
            let start = now.date_naive().and_hms_opt(0, 0, 0).unwrap().and_utc();
            Some((start, end))
        } else {
            None
        }
    }
}
