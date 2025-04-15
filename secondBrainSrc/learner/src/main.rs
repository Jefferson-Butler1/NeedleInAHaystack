use activity_tracker_common::{EventStore, TimescaleClient, UserEvent};
use std::error::Error;
use tokio::time::{interval, Duration};

mod keylogger;

use keylogger::Keylogger;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    //@todo put in .env variable
    let client =
        TimescaleClient::new("postgres://postgres:postgres@localhost:5432/postgres").await?;

    let keylogger = Keylogger::new();

    let mut interval = interval(Duration::from_secs(1));

    println!("Starting keylogger...");

    loop {
        interval.tick().await;

        if let Some(key_event) = keylogger.poll() {
            client.store_event(key_event).await?;
        }
    }
}
