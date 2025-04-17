use activity_tracker_common::{
    db::{GeneralDbClient, SummaryStore, TimescaleClient, TimescaleSummaryStore},
    llm::LlmClient,
    ActivitySummary,
};
use chrono::{DateTime, Datelike, Duration, Utc};
use std::error::Error;
use std::sync::Arc;
use tracing::{info, warn};

pub struct QueryEngine {
    sqlite_db: GeneralDbClient,
    timescale_db: TimescaleClient,
    llm_client: Arc<dyn LlmClient + Send + Sync>,
}

impl QueryEngine {
    pub fn new(
        sqlite_db: GeneralDbClient,
        timescale_db: TimescaleClient,
        llm_client: Arc<dyn LlmClient + Send + Sync>,
    ) -> Self {
        Self {
            sqlite_db,
            timescale_db,
            llm_client,
        }
    }

    pub async fn summarize_summaries(
        &self,
        user_prompt: &str,
        summaries: &[ActivitySummary],
    ) -> Result<String, Box<dyn Error + Send + Sync>> {
        if summaries.is_empty() {
            return Ok("No activities found for the specified period.".to_string());
        }

        info!(
            "Generating summary for {} activities with prompt: {}",
            summaries.len(),
            user_prompt
        );

        // Format summaries for better context
        let formatted_summaries = summaries
            .iter()
            .map(|s| {
                format!(
                    "Time: {} to {}\nDescription: {}\nTags: {}",
                    s.start_time.format("%H:%M:%S"),
                    s.end_time.format("%H:%M:%S"),
                    s.description,
                    s.tags.join(", ")
                )
            })
            .collect::<Vec<_>>()
            .join("\n---\n");

        // Craft a prompt for the LLM
        let prompt = format!(
            r#"You are analyzing a user's daily timeline of activities based on the summaries below.

User Query: "{}"

Timeline of activities:
{}

Based on these activities and the user's query, provide a concise and helpful analysis. 
Focus on answering the query specifically and extract any relevant patterns or insights.
Keep your response clear, direct, and limited to 3-5 sentences unless more detail is necessary.
"#,
            user_prompt, formatted_summaries
        );

        match self.llm_client.generate_text(&prompt).await {
            Ok(result) => Ok(result),
            Err(e) => Err(Box::new(std::io::Error::new(
                std::io::ErrorKind::Other,
                e.to_string(),
            )) as Box<dyn Error + Send + Sync>),
        }
    }

    pub async fn interpret_query(
        &self,
        query: &str,
    ) -> Result<String, Box<dyn Error + Send + Sync>> {
        // Use LLM to interpret natural language queries
        info!("Interpreting natural language query: {}", query);

        let prompt = format!(
            r#"You are interpreting a user's query to a Second Brain system that tracks their digital activities.

User Query: "{}"

Your task is to identify the type of information the user is looking for. It could be:
1. A time-based query (like "show me what I did yesterday" or "my coding sessions last week")
2. A topic-based query (like "find my work on the database project" or "when did I write emails to Sarah")
3. A pattern analysis (like "how much time did I spend coding vs meetings today")

Extract and return ONLY the key search terms and time frames from this query, keeping the essential meaning.
Format your response as a concise search string of 1-2 sentences. Do not add explanations.
"#,
            query
        );

        match self.llm_client.generate_text(&prompt).await {
            Ok(result) => Ok(result),
            Err(e) => Err(Box::new(std::io::Error::new(
                std::io::ErrorKind::Other,
                e.to_string(),
            )) as Box<dyn Error + Send + Sync>),
        }
    }

    pub async fn process_query(
        &self,
        query: &str,
    ) -> Result<Vec<ActivitySummary>, Box<dyn Error + Send + Sync>> {
        // Try with the original query first
        let direct_results = self.process_query_internal(query).await?;

        // If we got results, use them
        if !direct_results.is_empty() {
            info!("Found {} results with direct query", direct_results.len());
            return Ok(direct_results);
        }

        // If no direct results, try with an interpreted query
        info!("No results found with direct query, attempting to interpret");
        let interpreted_query = self.interpret_query(query).await?;
        info!("Interpreted query: '{}'", interpreted_query);

        // Try again with the interpreted query
        let interpreted_results = self.process_query_internal(&interpreted_query).await?;
        if !interpreted_results.is_empty() {
            info!(
                "Found {} results with interpreted query",
                interpreted_results.len()
            );
        } else {
            warn!("No results found with interpreted query either");
        }

        Ok(interpreted_results)
    }

    async fn process_query_internal(
        &self,
        query: &str,
    ) -> Result<Vec<ActivitySummary>, Box<dyn Error + Send + Sync>> {
        // Parse the time range or search term from the query
        if let Some(time_range) = self.parse_time_query(query) {
            // Compile a timeline for the day
            info!(
                "Processing as a time-based query: {} to {}",
                time_range.0, time_range.1
            );
            self.get_day_timeline(time_range.0).await
        } else {
            // Search both databases for matching summaries
            info!("Processing as a keyword search query: {}", query);
            let mut results = Vec::new();

            // Search SQLite summaries
            if let Ok(sqlite_results) = self.sqlite_db.search_summaries(query).await {
                info!("Found {} results in SQLite database", sqlite_results.len());
                results.extend(sqlite_results);
            }

            // Search TimescaleDB summaries and convert to ActivitySummary format
            if let Ok(timescale_results) = self.timescale_db.search_timescale_summaries(query).await
            {
                info!(
                    "Found {} results in TimescaleDB database",
                    timescale_results.len()
                );
                for (start_time, end_time, description, tags) in timescale_results {
                    results.push(ActivitySummary {
                        start_time,
                        end_time,
                        description,
                        tags,
                        events: vec![], // We don't store events in timescale summaries
                    });
                }
            }

            // Sort results by start time (newest first)
            results.sort_by(|a, b| b.start_time.cmp(&a.start_time));

            Ok(results)
        }
    }

    async fn get_day_timeline(
        &self,
        day: DateTime<Utc>,
    ) -> Result<Vec<ActivitySummary>, Box<dyn Error + Send + Sync>> {
        // Get all summaries for the day from both databases
        let mut timeline = Vec::new();

        // Get summaries from SQLite
        let day_start = day.date_naive().and_hms_opt(0, 0, 0).unwrap().and_utc();
        let day_end = day.date_naive().and_hms_opt(23, 59, 59).unwrap().and_utc();

        info!("Fetching day timeline from {} to {}", day_start, day_end);

        if let Ok(sqlite_results) = self
            .sqlite_db
            .get_summaries_in_timeframe(day_start, day_end)
            .await
        {
            info!(
                "Found {} summaries in SQLite for the day",
                sqlite_results.len()
            );
            timeline.extend(sqlite_results);
        }

        // Get summaries from TimescaleDB
        if let Ok(timescale_results) = self.timescale_db.get_timescale_summaries_for_day(day).await
        {
            info!(
                "Found {} summaries in TimescaleDB for the day",
                timescale_results.len()
            );
            for (start_time, end_time, description, tags) in timescale_results {
                timeline.push(ActivitySummary {
                    start_time,
                    end_time,
                    description,
                    tags,
                    events: vec![], // We don't store events in timescale summaries
                });
            }
        }

        // Sort timeline by start time (chronological order)
        timeline.sort_by(|a, b| a.start_time.cmp(&b.start_time));

        Ok(timeline)
    }

    fn parse_time_query(&self, query: &str) -> Option<(DateTime<Utc>, DateTime<Utc>)> {
        let now = Utc::now();
        let query = query.to_lowercase();

        if query.contains("last week") {
            let end = now;
            let start = now - Duration::days(7);
            Some((start, end))
        } else if query.contains("yesterday") {
            let yesterday = now - Duration::days(1);
            let start = yesterday
                .date_naive()
                .and_hms_opt(0, 0, 0)
                .unwrap()
                .and_utc();
            let end = yesterday
                .date_naive()
                .and_hms_opt(23, 59, 59)
                .unwrap()
                .and_utc();
            Some((start, end))
        } else if query.contains("today") {
            let start = now.date_naive().and_hms_opt(0, 0, 0).unwrap().and_utc();
            let end = now;
            Some((start, end))
        } else if query.contains("morning") {
            let start = now.date_naive().and_hms_opt(6, 0, 0).unwrap().and_utc();
            let end = now.date_naive().and_hms_opt(12, 0, 0).unwrap().and_utc();
            Some((start, end))
        } else if query.contains("afternoon") {
            let start = now.date_naive().and_hms_opt(12, 0, 0).unwrap().and_utc();
            let end = now.date_naive().and_hms_opt(18, 0, 0).unwrap().and_utc();
            Some((start, end))
        } else if query.contains("evening") {
            let start = now.date_naive().and_hms_opt(18, 0, 0).unwrap().and_utc();
            let end = now.date_naive().and_hms_opt(23, 59, 59).unwrap().and_utc();
            Some((start, end))
        } else if query.contains("this week") {
            // Start from the beginning of the current week (Monday)
            let days_since_monday = now.weekday().num_days_from_monday() as i64;
            let start = (now - Duration::days(days_since_monday))
                .date_naive()
                .and_hms_opt(0, 0, 0)
                .unwrap()
                .and_utc();
            let end = now;
            Some((start, end))
        } else if query.contains("last hour") {
            let start = now - Duration::hours(1);
            let end = now;
            Some((start, end))
        } else {
            None
        }
    }
}

// Make QueryEngine cloneable
impl Clone for QueryEngine {
    fn clone(&self) -> Self {
        Self {
            sqlite_db: self.sqlite_db.clone(),
            timescale_db: self.timescale_db.clone(),
            llm_client: self.llm_client.clone(),
        }
    }
}

