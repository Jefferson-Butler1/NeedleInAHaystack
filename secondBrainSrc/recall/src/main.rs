use activity_tracker_common::{
    db::GeneralDbClient,
    ActivitySummary,
};
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
    let listener = TcpListener::bind("127.0.0.1:8080").await?;
    println!("Recall thread started. Listening on 127.0.0.1:8080");

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
            Ok(summaries) => format_summaries(summaries, &query),
            Err(e) => format!("Error in fuzzy search: {}", e),
        }
    } else {
        match query_engine.process_query(&query).await {
            Ok(summaries) => format_summaries(summaries, &query),
            Err(e) => format!("Error in query: {}", e),
        }
    };

    let _ = socket.write_all(response.as_bytes()).await;
}

fn format_summaries(summaries: Vec<ActivitySummary>, query: &str) -> String {
    if summaries.is_empty() {
        return "Fishy says: I don't remember anything matching that query.".to_string();
    }

    let mut result = String::from("Fishy says:\n");
    
    // Identify query type
    let query_lower = query.to_lowercase();
    let is_key_query = query_lower.contains("key") && 
                      (query_lower.contains("most") || query_lower.contains("frequent"));
    let is_app_query = (query_lower.contains("app") || query_lower.contains("application")) && 
                       (query_lower.contains("most") || query_lower.contains("frequent"));
    
    for s in summaries {
        // Format the time
        let time_str = format!("{} to {}", 
            s.start_time.format("%H:%M"),
            s.end_time.format("%H:%M"));
        
        // Extract the most used keys
        let mut key_counts = std::collections::HashMap::new();
        for event in &s.events {
            if let Ok(data) = serde_json::from_str::<serde_json::Value>(&event.data) {
                if let Some(key) = data.get("key").and_then(|k| k.as_str()) {
                    *key_counts.entry(key.to_string()).or_insert(0) += 1;
                }
            }
        }
        
        let mut key_vec: Vec<_> = key_counts.into_iter().collect();
        key_vec.sort_by(|a, b| b.1.cmp(&a.1));
        
        // Extract the most used apps
        let mut app_counts = std::collections::HashMap::new();
        for event in &s.events {
            let app_name = if event.app_context.app_name.contains("ghostty") {
                "Terminal".to_string()
            } else if event.app_context.app_name.contains("firefox") {
                "Firefox".to_string()
            } else {
                event.app_context.app_name.clone()
            };
            
            *app_counts.entry(app_name).or_insert(0) += 1;
        }
        
        let mut app_vec: Vec<_> = app_counts.into_iter().collect();
        app_vec.sort_by(|a, b| b.1.cmp(&a.1));
        
        // Generate appropriate response based on query type
        if is_key_query {
            // Format keys response
            let mut response = String::new();
            for (i, (key, count)) in key_vec.iter().take(10).enumerate() {
                response.push_str(&format!("{}. {} ({} times)\n", i+1, key, count));
            }
            
            result.push_str(&format!(
                "• {}: Most frequently used keys:\n{} ({} events)\n",
                time_str,
                response,
                s.events.len()
            ));
        } else if is_app_query {
            // Format apps response
            let mut response = String::new();
            for (i, (app, count)) in app_vec.iter().take(5).enumerate() {
                response.push_str(&format!("{}. {} ({} events)\n", i+1, app, count));
            }
            
            result.push_str(&format!(
                "• {}: Most frequently used applications:\n{} ({} events)\n",
                time_str,
                response,
                s.events.len()
            ));
        } else {
            // For regular activity queries
            let apps: Vec<String> = app_vec.iter()
                .take(2)
                .map(|(name, _)| name.clone())
                .collect();
            
            let apps_str = format!(" in {}", apps.join(" and "));
            
            // Determine appropriate description
            let activity_desc = if key_vec.iter().any(|(k, _)| k == "Escape" || k == "KeyI" || k == "KeyA") &&
                                   apps.contains(&"Terminal".to_string()) {
                "Editing text with Vim, switching between insert and normal modes".to_string()
            } else if key_vec.iter().any(|(k, _)| k == "UpArrow" || k == "DownArrow") &&
                        key_vec.iter().any(|(k, _)| k == "Return") &&
                        apps.contains(&"Terminal".to_string()) {
                "Navigating through command history in the terminal".to_string()
            } else if apps.contains(&"Firefox".to_string()) &&
                        key_vec.iter().any(|(k, _)| k == "KeyS" || k == "KeyF") {
                "Searching for information on web pages".to_string()
            } else if key_vec.iter().any(|(k, _)| k.starts_with("Key")) &&
                        key_vec.iter().any(|(k, _)| k == "Space" || k == "Return") {
                "Writing or editing text".to_string()
            } else {
                "Working on the computer".to_string()
            };
            
            result.push_str(&format!(
                "• {}: {}{} ({} events)\n",
                time_str,
                activity_desc,
                apps_str,
                s.events.len()
            ));
        }
    }
    
    result
}
