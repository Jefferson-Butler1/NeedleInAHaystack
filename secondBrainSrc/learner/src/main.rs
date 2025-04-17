use activity_tracker_common::{db::EventStore, db::TimescaleClient};
use std::error::Error;
use tokio::time::{interval, Duration};
use dotenv::dotenv;
use std::env;

mod keylogger;

use keylogger::Keylogger;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // Load environment variables from .env file if present
    dotenv().ok();
    
    // Get connection string from environment or use default
    let db_url = env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:postgres@localhost:5435/second_brain".to_string());
    
    println!("Connecting to database...");
    let client = TimescaleClient::new(&db_url).await?;
    
    let keylogger = Keylogger::new();
    
    // Poll interval (in seconds)
    let poll_interval = env::var("POLL_INTERVAL")
        .ok()
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(1);
    
    let mut interval = interval(Duration::from_secs(poll_interval));
    
    println!("Starting keylogger with poll interval of {}s...", poll_interval);
    
    loop {
        interval.tick().await;
        
        if let Some(key_event) = keylogger.poll() {
            println!("Captured event: {:?}", key_event);
            match client.store_event(key_event).await {
                Ok(_) => {},
                Err(e) => eprintln!("Error storing event: {}", e),
            }
        }
    }
}
