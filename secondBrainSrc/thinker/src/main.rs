use activity_tracker_common::{TimescaleClient, GeneralDbClient, EventStore, SummaryStore, ActivitySummary};
use chrono:: {Utc, Duration};
use std::error::Error;
use tokio::time::{interval, Duration as TokioDuration};

mod event_analyzer;
mod llm_client;

use event_analyzer::EventAnalyzer;
use llm_client::LLMClient;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let events_db = TimescaleClient::new("postgres://postgres:password@localhost:5432/activity_data").await?;
    let summary_db = GeneralDbClient::new("mongodb://localhost:27017")?;

    //@todo get llm api key for this!
    let llm_client = LLMClient::new("API_KEY_HERE").await?;

    let analyzer = EventAnalyzer::new(llm_client);

    let mut interval = interval(TokioDuration::from_secs(300));

    println!("Thinker thread started. Processing at 5 minute intervals...");

    loop {
        interval.tick().await;

        let end_time = Utc::now();
        let start_time = end_time - Duration::minutes(5);

        let events = events_db.get_events_in_timeframe(start_time, end_time).await?;

        if !events.is_empty() {
            let summaries = analyzer.analyze_events(events, start_time, end_time).await?;
            summary_db.store_summary(summary).await?;
        }
    }
}
