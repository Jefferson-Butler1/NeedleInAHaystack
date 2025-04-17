use activity_tracker_common::{
    db::{EventStore, TimescaleClient, TimescaleSummaryStore},
    llm::create_default_client,
};
use chrono::{Duration, Utc};
use dotenv::dotenv;
use std::error::Error;
use std::env;
use tokio::time::{interval, Duration as TokioDuration};
use tracing::{info, error, warn, Level};
use tracing_subscriber::FmtSubscriber;

mod event_analyzer;
use event_analyzer::EventAnalyzer;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // Initialize tracing
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(subscriber)
        .expect("Failed to set tracing subscriber");

    // Load environment variables
    dotenv().ok();
    
    // Get database connection strings from environment variables
    let events_db_url = env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:postgres@localhost:5435/second_brain".to_string());
    
    // Connect to database
    info!("Connecting to database...");
    let db_client = TimescaleClient::new(&events_db_url).await?;
    
    // Initialize LLM client
    info!("Initializing LLM client...");
    let llm_client = create_default_client().await?;
    
    // Create analyzer
    let analyzer = EventAnalyzer::new(llm_client);
    
    // Setup processing interval (1 minute)
    let interval_secs = 60; // Process events every minute
    
    let mut interval = interval(TokioDuration::from_secs(interval_secs));
    
    info!("Thinker thread started. Processing every 60 seconds.");
    
    loop {
        interval.tick().await;
        
        let end_time = Utc::now();
        let start_time = end_time - Duration::minutes(5);
        
        info!("Analyzing events from {} to {}", start_time, end_time);
        
        // Get events from the last 5 minutes
        match db_client.get_events_in_timeframe(start_time, end_time).await {
            Ok(events) => {
                if !events.is_empty() {
                    info!("Found {} events to analyze", events.len());
                    
                    // Analyze events
                    match analyzer.analyze_events(events, start_time, end_time).await {
                        Ok((description, tags, keystrokes)) => {
                            // Store summary in timescale db
                            match db_client.store_timescale_summary(
                                start_time,
                                end_time,
                                description.clone(),
                                tags.clone(),
                                keystrokes
                            ).await {
                                Ok(_) => info!("Successfully stored summary"),
                                Err(e) => error!("Failed to store summary: {}", e),
                            }
                        },
                        Err(e) => error!("Failed to analyze events: {}", e),
                    }
                } else {
                    warn!("No events found in the specified time period");
                }
            },
            Err(e) => error!("Failed to retrieve events: {}", e),
        }
    }
}