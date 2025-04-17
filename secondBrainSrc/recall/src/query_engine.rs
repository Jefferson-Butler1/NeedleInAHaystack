use activity_tracker_common::{
    db::{GeneralDbClient, SummaryStore, TimescaleClient, TimescaleSummaryStore},
    ActivitySummary,
};
use chrono::{DateTime, Duration, Utc};
use std::error::Error;

#[derive(Clone)]
pub struct QueryEngine {
    sqlite_db: GeneralDbClient,
    timescale_db: TimescaleClient,
}

impl QueryEngine {
    pub fn new(sqlite_db: GeneralDbClient, timescale_db: TimescaleClient) -> Self {
        Self {
            sqlite_db,
            timescale_db,
        }
    }

    pub async fn process_query(&self, query: &str) -> Result<Vec<ActivitySummary>, Box<dyn Error>> {
        // Parse the time range or search term from the query
        if let Some(time_range) = self.parse_time_query(query) {
            // Compile a timeline for the day
            self.get_day_timeline(time_range.0).await
        } else {
            // Search both databases for matching summaries
            let mut results = Vec::new();

            // Search SQLite summaries
            if let Ok(sqlite_results) = self.sqlite_db.search_summaries(query).await {
                results.extend(sqlite_results);
            }

            // Search TimescaleDB summaries and convert to ActivitySummary format
            if let Ok(timescale_results) = self.timescale_db.search_timescale_summaries(query).await
            {
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
    ) -> Result<Vec<ActivitySummary>, Box<dyn Error>> {
        // Get all summaries for the day from both databases
        let mut timeline = Vec::new();

        // Get summaries from SQLite
        let day_start = day.date_naive().and_hms_opt(0, 0, 0).unwrap().and_utc();
        let day_end = day.date_naive().and_hms_opt(23, 59, 59).unwrap().and_utc();

        if let Ok(sqlite_results) = self
            .sqlite_db
            .get_summaries_in_timeframe(day_start, day_end)
            .await
        {
            timeline.extend(sqlite_results);
        }

        // Get summaries from TimescaleDB
        if let Ok(timescale_results) = self.timescale_db.get_timescale_summaries_for_day(day).await
        {
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
        } else {
            None
        }
    }
}

