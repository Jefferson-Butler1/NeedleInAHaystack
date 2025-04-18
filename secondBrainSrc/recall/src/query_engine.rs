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
        // If it's a time-based query, handle it directly
        if let Some(time_range) = self.parse_time_query(query) {
            return self.db_client.get_summaries_in_timeframe(time_range.0, time_range.1).await;
        }
        
        // Otherwise, sanitize the query and perform a search
        let clean_query = self.sanitize_query_for_fts(query);
        self.db_client.search_summaries(&clean_query).await
    }

    // Add this new method to sanitize queries
    fn sanitize_query_for_fts(&self, query: &str) -> String {
        // Remove question marks and other special characters
        let clean_query = query.chars()
            .filter(|c| c.is_alphanumeric() || c.is_whitespace())
            .collect::<String>();
    
        // Extract key terms (simplified approach - split by spaces and take words 3+ chars)
        let terms = clean_query.split_whitespace()
            .filter(|word| word.len() >= 3)
            .collect::<Vec<_>>();
    
        if terms.is_empty() {
            "user activity".to_string() // Fallback search term
        } else {
            terms.join(" ") // Join with OR for more permissive matching
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
