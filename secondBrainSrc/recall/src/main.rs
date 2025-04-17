use activity_tracker_common::{db::GeneralDbClient, ActivitySummary};
use dotenv::dotenv;
use std::env;
use std::error::Error;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

mod fuzzy_finder;
mod query_engine;

use fuzzy_finder::FuzzyFinder;
use query_engine::QueryEngine;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // Load environment variables
    dotenv().ok();

    // Get the summary database URL from environment
    let summary_db_url =
        env::var("SUMMARY_DB_URL").unwrap_or_else(|_| "sqlite:./data/summaries.db".to_string());

    println!("🔌 Connecting to summary database...");
    let db_client = GeneralDbClient::new(&summary_db_url).await?;
    println!("✅ Connected to summary database");

    let query_engine = QueryEngine::new(db_client.clone());
    let fuzzy_finder = FuzzyFinder::new(db_client);

    // @todo setup a simple TCP server to handle recall requests
    let listener = TcpListener::bind("127.0.0.1:8081").await?;
    println!("Recall thread started. Listening on 127.0.0.1:8081");

    loop {
        let (socket, _) = listener.accept().await?;

        let query_engine = query_engine.clone();
        let fuzzy_finder = fuzzy_finder.clone();

        // Process a client request in a new task
        tokio::spawn(async move {
            handle_client(socket, query_engine, fuzzy_finder).await;
        });

        println!("Recall thread is running...");
    }
}

// Separate function to handle client connections
async fn handle_client(
    mut socket: tokio::net::TcpStream,
    query_engine: QueryEngine,
    fuzzy_finder: FuzzyFinder,
) {
    let mut buffer = [0; 1024];

    // Read from the socket
    let n = match socket.read(&mut buffer).await {
        Ok(n) => n,
        Err(e) => {
            println!("Error reading from socket: {}", e);
            return;
        }
    };

    // Convert bytes to string
    let query = String::from_utf8_lossy(&buffer[..n]).to_string();

    // Process the query and immediately convert to a response string
    let response = if query.starts_with("Fuzzy:") {
        let search_term = &query[6..];
        match fuzzy_finder.search(search_term).await {
            Ok(summaries) => format_summaries(summaries),
            Err(e) => format!("Error in fuzzy search: {}", e),
        }
    } else {
        match query_engine.process_query(&query).await {
            Ok(summaries) => format_summaries(summaries),
            Err(e) => format!("Error in query: {}", e),
        }
    };

    // No need to handle this error since we're about to close the connection anyway
    let _ = socket.write_all(response.as_bytes()).await;
}

fn format_summaries(summaries: Vec<ActivitySummary>) -> String {
    summaries
        .iter()
        .map(|s| {
            let event_count = s.events.len();

            format!(
                "Time: {} to {} \nDescription: {}\nTags: {}\nEvent count: {}\n",
                s.start_time,
                s.end_time,
                s.description,
                s.tags.join(", "),
                event_count
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}
