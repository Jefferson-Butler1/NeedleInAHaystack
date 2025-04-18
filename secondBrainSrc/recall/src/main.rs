use activity_tracker_common::{
    db::GeneralDbClient,
    llm,
    llm::LlmClient,
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
async fn main() -> Result<(), Box<dyn Error + Send + Sync>> {
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

    // Setup a simple TCP server to handle recall requests
    let listener = TcpListener::bind("127.0.0.1:8080").await?;
    println!("🚀 Recall thread started. Listening on 127.0.0.1:8080");

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

    // Process the query and generate a response string
    let response = process_query(&query, &query_engine, &fuzzy_finder).await;
    let _ = socket.write_all(response.as_bytes()).await;
}

// Process the query and generate a response
async fn process_query(
    query: &str, 
    query_engine: &QueryEngine, 
    fuzzy_finder: &FuzzyFinder
) -> String {
    if query.starts_with("Fuzzy:") {
        let search_term = &query[6..];
        match fuzzy_finder.search(search_term).await {
            Ok(summaries) => {
                let result = format_summaries_with_ai(summaries, query).await;
                result
            },
            Err(e) => format!("Error in fuzzy search: {}", e),
        }
    } else {
        match query_engine.process_query(query).await {
            Ok(summaries) => {
                let result = format_summaries_with_ai(summaries, query).await;
                result
            },
            Err(e) => format!("Error in query: {}", e),
        }
    }
}

// New function that adds AI distillation before formatting
async fn format_summaries_with_ai(summaries: Vec<ActivitySummary>, query: &str) -> String {
    if summaries.is_empty() {
        return "Fishy says: I don't remember anything matching that query.".to_string();
    }

    // Prepare summaries data for AI processing
    let raw_summary = format_summaries_raw(summaries, query);
    
    // Use LLM to create a markdown table summary
    match distill_with_ai(&raw_summary, query).await {
        Ok(ai_summary) => {
            format!("Fishy says:\n\n{}", ai_summary)
        }
        Err(e) => {
            eprintln!("Error with AI distillation: {}", e);
            // Fall back to regular formatting if AI fails
            format!("Fishy says:\n{}", raw_summary)
        }
    }
}

// Helper function to generate raw summaries for AI processing
fn format_summaries_raw(summaries: Vec<ActivitySummary>, query: &str) -> String {
    let mut result = String::new();
    
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
        
        // Extract the most used keys and apps only when specifically asked about them
        if is_key_query || is_app_query {
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
            }
        } else {
            // For regular activity queries, use the description from the summary
            // Get apps for context without showing detailed stats
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
            
            let apps: Vec<String> = app_vec.iter()
                .take(2)
                .map(|(name, _)| name.clone())
                .collect();
            
            let apps_str = if !apps.is_empty() {
                format!(" in {}", apps.join(" and "))
            } else {
                String::new()
            };
            
            // Clean up the description to remove any code-related content
            let description = s.description.lines()
                .take(3)
                .filter(|line| !line.contains("```") && 
                             !line.contains("script") && 
                             !line.contains("parse") && 
                             !line.contains("code") && 
                             !line.contains("example") &&
                             !line.contains("analyze this data"))
                .collect::<Vec<_>>()
                .join("\n");
            
            result.push_str(&format!(
                "• {}: {}{} ({} events)\n",
                time_str,
                description,
                apps_str,
                s.events.len()
            ));
        }
    }
    
    result
}

// Use LLM to create a more concise, structured summary in markdown table format
async fn distill_with_ai(raw_summary: &str, query: &str) -> Result<String, Box<dyn Error + Send + Sync>> {
    let llm_client = llm::create_default_client().await?;
    
    let prompt = format!(
        "I have user activity data that answers the query: '{}'. Here is the raw data:\n\n{}\n\n
        Create a concise, clearly formatted response with the following structure:
        
        1. A brief 1-2 sentence summary of what the user was doing
        2. A well-formatted markdown table summarizing the key information
        3. Any notable patterns or insights (optional, only if clear from the data)
        
        For the markdown table:
        - Format with proper | --- | syntax for headers
        - Choose appropriate columns based on the data type
        - For key/app frequency queries: use columns like Rank, Item, Count
        - For activity queries: use columns like Time, Activity, Apps
        - Limit to 5-7 rows maximum to keep it readable
        
        Keep the entire response concise and visually clean.",
        query, raw_summary
    );

    let response = llm_client.generate_text(&prompt).await?;
    Ok(response)
}