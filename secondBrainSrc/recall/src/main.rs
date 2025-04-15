use activity_tracker_common::{ActivitySummary, GeneralDbClient, SummaryStore};
use std::error::Error;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

mod fuzzy_finder;
mod query_engine;

use fuzzy_finder::FuzzyFinder;
use query_engine::QueryEngine;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let db_client = GeneralDbClient::new("mongodb://localhost:27017")?;

    let query_engine = QueryEngine::new(db_client.clone());
    let fuzzy_finder = FuzzyFinder::new(db_client);

    // @todo setup a simple TCP server to handle recall requests
    let listener = TcpListener::bind("127.0.0.1:8080").await?;
    println!("Recall thread started. Listening on 127.0.0.1:8080");

    loop {
        let (mut socket, _) = listener.accept().await?;

        let query_engine = query_engine.clone();
        let fuzzy_finder = fuzzy_finder.clone();

        tokio::spawn(async move {
            let mut buffer = [0; 1024];

            match socket.read(&mut buffer).await {
                Ok(n) => {
                    let query = String::from_utf8_lossy(&buffer[..n]).to_string();

                    let result = if query.starts_with("Fuzzy:") {
                        let search_term = &query[6..];
                        fuzzy_finder.search(search_term).await
                    } else {
                        query_engine.process_query(&query).await
                    };

                    match result {
                        Ok(summaries) => {
                            let response = format_summaries(summaries);
                            let _ = socket.write_all(response.as_bytes()).await;
                        }
                        Err(e) => {
                            let _ = socket.write_all(format!("Error: {}", e).as_bytes()).await;
                        }
                    }
                }
                Err(e) => {
                    println!("Error reading from socket: {}", e);
                }
            }
        });
        println!("Recall thread is running...");
    }
}

fn format_summaries(summaries: Vec<ActivitySummary>) -> String {
    summaries
        .iter()
        .map(|s| {
            format!(
                "Time: {} to {} \nDescription: {}\nTags: {}\nEvents: {}\n",
                s.start_time,
                s.end_time,
                s.description,
                s.tags.join(", "),
                s.events.join(", ")
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}
