use activity_tracker_common::{db::GeneralDbClient, db::TimescaleClient, ActivitySummary};
use dotenv::dotenv;
use std::env;
use std::error::Error;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tracing::{error, info, Level};
use tracing_subscriber::FmtSubscriber;

mod fuzzy_finder;
mod query_engine;

use fuzzy_finder::FuzzyFinder;
use query_engine::QueryEngine;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // Initialize tracing
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(subscriber).expect("Failed to set tracing subscriber");

    // Load environment variables
    dotenv().ok();

    // Get the database URLs from environment
    let summary_db_url =
        env::var("SUMMARY_DB_URL").unwrap_or_else(|_| "sqlite:./data/summaries.db".to_string());

    let timescale_db_url = env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:postgres@localhost:5435/second_brain".to_string());

    // Connect to databases
    info!("Connecting to summary database (SQLite)...");
    let sqlite_client = GeneralDbClient::new(&summary_db_url).await?;

    info!("Connecting to events database (TimescaleDB)...");
    let timescale_client = TimescaleClient::new(&timescale_db_url).await?;

    // Initialize query engine and fuzzy finder
    let query_engine = QueryEngine::new(sqlite_client.clone(), timescale_client.clone());
    let fuzzy_finder = FuzzyFinder::new(sqlite_client);

    // Setup TCP server for handling recall requests
    let port = env::var("RECALL_PORT").unwrap_or_else(|_| "8081".to_string());
    let addr = format!("127.0.0.1:{}", port);
    let listener = TcpListener::bind(&addr).await?;
    info!("Recall thread started. Listening on {}", addr);

    loop {
        match listener.accept().await {
            Ok((socket, _)) => {
                let query_engine = query_engine.clone();
                let fuzzy_finder = fuzzy_finder.clone();

                // Process each client request in a separate task
                tokio::spawn(async move {
                    if let Err(e) = handle_client(socket, query_engine, fuzzy_finder).await {
                        error!("Error handling client: {}", e);
                    }
                });
            }
            Err(e) => {
                error!("Failed to accept connection: {}", e);
            }
        }
    }
}

// Handle client connections
async fn handle_client(
    mut socket: tokio::net::TcpStream,
    query_engine: QueryEngine,
    fuzzy_finder: FuzzyFinder,
) -> Result<(), Box<dyn Error>> {
    let mut buffer = [0; 8192]; // Increased buffer size to 8KB

    // Read from the socket
    let n = socket.read(&mut buffer).await?;

    if n == 0 {
        return Ok(());
    }

    // Convert bytes to string
    let query = String::from_utf8_lossy(&buffer[..n]).to_string();
    info!("Received query: {}", query.trim());

    // Check if this is an HTTP request
    let response = if query.starts_with("GET")
        || query.starts_with("POST")
        || query.starts_with("HTTP")
    {
        // Send a simple HTTP response
        format!("HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nConnection: close\r\n\r\nSecond Brain Recall Service\r\n\r\nUsage:\r\n- Send plain text queries to search your activity\r\n- Start with 'Fuzzy:' for fuzzy searching\r\n- Try queries like 'today', 'yesterday', 'morning', 'evening'")
    } else if query.starts_with("Fuzzy:") {
        let search_term = &query[6..];
        match fuzzy_finder.search(search_term).await {
            Ok(summaries) => format_summaries(summaries),
            Err(e) => format!("Error in fuzzy search: {}", e),
        }
    } else {
        // Extract actual query if there might be HTTP headers
        let actual_query = query.lines().next().unwrap_or(&query);
        match query_engine.process_query(actual_query).await {
            Ok(summaries) => format_summaries(summaries),
            Err(e) => format!("Error in query: {}", e),
        }
    };

    // Send response back to client
    socket.write_all(response.as_bytes()).await?;

    Ok(())
}

fn format_summaries(summaries: Vec<ActivitySummary>) -> String {
    if summaries.is_empty() {
        return "No matching summaries found.".to_string();
    }

    summaries
        .iter()
        .map(|s| {
            let time_fmt =
                |dt: chrono::DateTime<chrono::Utc>| -> String { dt.format("%H:%M:%S").to_string() };

            format!(
                "Time: {} to {}\nDescription: {}\nTags: {}\n",
                time_fmt(s.start_time),
                time_fmt(s.end_time),
                s.description,
                s.tags.join(", ")
            )
        })
        .collect::<Vec<_>>()
        .join("\n---------------\n")
}
