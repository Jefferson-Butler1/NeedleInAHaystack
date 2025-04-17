use activity_tracker_common::{db::EventStore, db::TimescaleClient};
use dotenv::dotenv;
use std::env;
use std::error::Error;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;
use tokio::time::{interval, Duration};

mod keylogger;

use keylogger::Keylogger;

// Constants
const DEFAULT_DB_URL: &str = "postgres://postgres:postgres@localhost:5435/second_brain";
const DEFAULT_POLL_INTERVAL: u64 = 1;
const STATS_INTERVAL: u64 = 60; // Print stats every 60 seconds

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // Load environment variables from .env file if present
    dotenv().ok();

    // Get connection string from environment or use default
    let db_url = env::var("DATABASE_URL").unwrap_or_else(|_| DEFAULT_DB_URL.to_string());

    // Poll interval (in seconds)
    let poll_interval = env::var("POLL_INTERVAL")
        .ok()
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(DEFAULT_POLL_INTERVAL);

    println!("🔄 Starting Second Brain Learner");
    println!("📊 Database: {}", db_url);
    println!("⏱️ Poll interval: {}s", poll_interval);

    // Connect to the database
    println!("🔌 Connecting to database...");
    let client = match TimescaleClient::new(&db_url).await {
        Ok(c) => {
            println!("✅ Database connection established");
            c
        }
        Err(e) => {
            eprintln!("❌ Database connection failed: {}", e);
            eprintln!(
                "⚠️ Please check your database settings and make sure TimescaleDB is running"
            );
            return Err(e);
        }
    };

    println!("🔑 Initializing keylogger...");
    let keylogger = Keylogger::new();
    println!("✅ Keylogger initialized");

    // Set up statistics trackers
    let total_events = AtomicUsize::new(0);
    let start_time = Instant::now();
    let mut stats_interval = interval(Duration::from_secs(STATS_INTERVAL));
    let mut poll_timer = interval(Duration::from_secs(poll_interval));

    println!("🚀 Learner is running. Press Ctrl+C to stop.");

    loop {
        tokio::select! {
            _ = poll_timer.tick() => {
                // Poll for keyboard events
                while let Some(key_event) = keylogger.poll() {
                    total_events.fetch_add(1, Ordering::Relaxed);

                    match client.store_event(key_event).await {
                        Ok(_) => {},
                        Err(e) => eprintln!("❌ Error storing event: {}", e),
                    }
                }
            }

            _ = stats_interval.tick() => {
                // Print statistics
                let elapsed = start_time.elapsed().as_secs();
                let events = total_events.load(Ordering::Relaxed);

                if elapsed > 0 {
                    let events_per_min = (events as f64 / elapsed as f64) * 60.0;
                    println!("📈 Stats: {} events captured ({:.2} events/min)", events, events_per_min);
                }
            }
        }
    }
}
