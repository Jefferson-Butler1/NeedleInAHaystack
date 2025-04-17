use activity_tracker_common::{
    db::GeneralDbClient, db::TimescaleClient, llm::create_default_client, ActivitySummary,
};
use dotenv::dotenv;
use std::env;
use std::error::Error;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Mutex;
use tracing::{error, info, warn, Level};
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

    // Initialize LLM client
    info!("Initializing LLM client...");
    let llm_client = create_default_client().await?;
    info!("LLM client initialized");

    // Initialize query engine and fuzzy finder
    info!("Setting up query engine...");
    let query_engine = Arc::new(QueryEngine::new(
        sqlite_client.clone(),
        timescale_client.clone(),
        Arc::new(llm_client),
    ));

    let fuzzy_finder = Arc::new(Mutex::new(FuzzyFinder::new(sqlite_client)));

    // Setup TCP server for handling recall requests
    let port = env::var("RECALL_PORT").unwrap_or_else(|_| "8081".to_string());
    let addr = format!("127.0.0.1:{}", port);
    let listener = TcpListener::bind(&addr).await?;
    info!("Recall thread started. Listening on {}", addr);

    loop {
        match listener.accept().await {
            Ok((socket, _)) => {
                let query_engine = Arc::clone(&query_engine);
                let fuzzy_finder = Arc::clone(&fuzzy_finder);

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
    mut socket: TcpStream,
    query_engine: Arc<QueryEngine>,
    fuzzy_finder: Arc<Mutex<FuzzyFinder>>,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let mut buffer = [0; 8192]; // Increased buffer size to 8KB

    // Read from the socket
    let n = socket.read(&mut buffer).await?;

    if n == 0 {
        return Ok(());
    }

    // Convert bytes to string
    let query = String::from_utf8_lossy(&buffer[..n]).to_string();
    info!("Received query: {}", query.trim());

    // Request for an LLM-summarized response
    let user_query = &query[10..]; // Remove the "Summarize:" prefix

    // First get the matching activities
    let response = match query_engine.process_query(user_query).await {
        Ok(summaries) if !summaries.is_empty() => {
            // Then generate an LLM summary
            println!(
                "query: {}\n summaries: {}",
                user_query,
                format_summaries(&summaries)
            );
            match query_engine
                .summarize_summaries(user_query, &summaries)
                .await
            {
                Ok(summary) => format!("Summary: {}", summary),
                Err(e) => format!("Error generating summary: {}", e),
            }
        }
        Ok(_) => "No activities found for the given query.".to_string(),
        Err(e) => format!("Error retrieving activities: {}", e),
    };

    // Send response back to client
    socket.write_all(response.as_bytes()).await?;

    Ok(())
}

fn format_summaries(summaries: &[ActivitySummary]) -> String {
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

