use activity_tracker_common::{
    db::{EventStore, GeneralDbClient, SummaryStore, TimescaleClient},
    llm::create_default_client,
};
use chrono::{Duration, Utc};
use dotenv::dotenv;
use std::error::Error;
use std::env;
use tokio::time::{interval, Duration as TokioDuration};

mod event_analyzer;
use event_analyzer::EventAnalyzer;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // Load environment variables
    dotenv().ok();
    
    // Get database connection strings from environment variables
    let events_db_url = env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:postgres@localhost:5435/second_brain".to_string());
    
    let summary_db_url = env::var("SUMMARY_DB_URL")
        .unwrap_or_else(|_| "sqlite:./data/summaries.db".to_string());
    
    // Connect to databases
    println!("🔌 Connecting to event database...");
    let events_db = TimescaleClient::new(&events_db_url).await?;
    
    println!("🔌 Connecting to summary database...");
    let summary_db = GeneralDbClient::new(&summary_db_url).await?;
    
    // Initialize LLM client
    println!("🧠 Initializing LLM client...");
    let llm_client = create_default_client().await?;
    println!("✅ LLM client initialized");
    
    // Create analyzer
    let analyzer = EventAnalyzer::new(llm_client);
    
    // Setup processing interval (5 minutes)
    let interval_secs = env::var("THINKER_INTERVAL_SECS")
        .ok()
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(300); // Default to 5 minutes
    
    let mut interval = interval(TokioDuration::from_secs(interval_secs));
    
    println!("🚀 Thinker thread started. Processing at {} second intervals...", interval_secs);
    
    loop {
        interval.tick().await;
        
        let end_time = Utc::now();
        let start_time = end_time - Duration::minutes(5);
        
        println!("🔍 Analyzing events from {} to {}", start_time, end_time);
        
        let events = events_db
            .get_events_in_timeframe(start_time, end_time)
            .await?;
        
        if !events.is_empty() {
            println!("📊 Found {} events to analyze", events.len());
            
            let summary = analyzer
                .analyze_events(events, start_time, end_time)
                .await?;
                
            println!("💾 Storing summary: {}", summary.description);
            summary_db.store_summary(&summary).await?;
        } else {
            println!("⚠️ No events found in the specified time period");
        }
    }
}
