use activity_tracker_common::{db::TimescaleClient, llm, llm::LlmClient, UserEvent};
use chrono::Utc;
use dotenv::dotenv;
use query_engine::{QueryEngine, QueryResult};
use sqlx::{postgres::PgPoolOptions, Pool, Postgres};
mod utils;
use std::collections::{HashMap, HashSet};
use std::env;
use std::error::Error;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::Mutex;

mod query_engine;

// Personality constants
const FISHY_INTRO: &[&str] = &[
    "🐠 Fishy splashes with excitement! ",
    "🐟 Fishy bubbles up with joy! ",
    "🐠 Fishy swims over with the info! ",
    "🐟 Fishy floats to the surface! ",
    "🐠 Fishy darts around happily! ",
];

const FISHY_NO_DATA: &[&str] = &[
    "🐠 Fishy looks confused... I don't seem to remember anything about that. Maybe try asking about a different time period?",
    "🐟 Fishy scratches his scales... I don't have any memory of that! Would you like to know about something else?",
    "🐠 Fishy does a sad little loop... I don't have any data matching your question. Try asking about today or yesterday!",
    "🐟 Fishy checks his tiny fish brain... No memories found! Perhaps I can tell you about something more recent?",
];

/// Generate a random fishy intro
fn random_fishy_intro() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let seed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    FISHY_INTRO[seed as usize % FISHY_INTRO.len()].to_string()
}

/// Generate a random fishy no-data message
fn random_fishy_no_data() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let seed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    FISHY_NO_DATA[seed as usize % FISHY_NO_DATA.len()].to_string()
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error + Send + Sync>> {
    // Load environment variables
    dotenv().ok();

    // Get database URL from environment
    let db_url = env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:postgres@localhost:5438/second_brain".to_string());

    println!("🔌 Connecting to PostgreSQL database...");
    // Connect to PostgreSQL
    let pg_pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&db_url)
        .await?;

    println!("✅ Connected to PostgreSQL database");

    // Create a shared reference to the PostgreSQL pool
    let pg_pool = Arc::new(pg_pool);

    // Also connect to the events database (which is the same in this case)
    println!("🔌 Setting up events database connection...");
    let events_db = match TimescaleClient::new(&db_url).await {
        Ok(db) => {
            println!("✅ Events database connection established");
            Some(Arc::new(db))
        }
        Err(e) => {
            println!("⚠️ Failed to connect to events database: {}", e);
            None
        }
    };

    // Create LLM client
    println!("🧠 Initializing LLM client...");
    let llm_client = match llm::create_default_client().await {
        Ok(client) => {
            println!("✅ LLM client initialized");
            // Convert to Box<dyn LlmClient + Send + Sync>
            let boxed_client: Box<dyn LlmClient + Send + Sync> = Box::new(client);
            Arc::new(Mutex::new(boxed_client))
        }
        Err(e) => {
            println!("⚠️ Failed to initialize LLM client: {}", e);
            return Err(e);
        }
    };

    let query_engine = QueryEngine::new(pg_pool, events_db);

    // Setup TCP server
    let listener = TcpListener::bind("127.0.0.1:8080").await?;
    println!("🚀 Recall service started. Listening on 127.0.0.1:8080");
    println!("🐠 Fishy is ready to help you remember things!");

    loop {
        let (socket, _) = listener.accept().await?;
        let query_engine = query_engine.clone();
        let llm_client = Arc::clone(&llm_client);

        // Process request in a new task
        tokio::spawn(async move {
            handle_client(socket, query_engine, llm_client).await;
        });
    }
}

/// Handle a client request
async fn handle_client(
    mut socket: tokio::net::TcpStream,
    query_engine: QueryEngine,
    llm_client: Arc<Mutex<Box<dyn LlmClient + Send + Sync>>>,
) {
    let mut buffer = [0; 4096]; // Increased buffer size for larger queries

    // Read from the socket
    let n = match socket.read(&mut buffer).await {
        Ok(n) => n,
        Err(e) => {
            eprintln!("Error reading from socket: {}", e);
            return;
        }
    };

    // Convert bytes to string and trim any whitespace
    let query = String::from_utf8_lossy(&buffer[..n]).trim().to_string();

    println!("📝 Received query: {}", query);

    // Process the query
    let response = process_query(&query, &query_engine, &llm_client).await;

    println!("✅ Sending response (length: {} chars)", response.len());

    // Send the response back
    if let Err(e) = socket.write_all(response.as_bytes()).await {
        eprintln!("Error writing to socket: {}", e);
    }
}

/// Process a query and generate a response
async fn process_query(
    query: &str,
    query_engine: &QueryEngine,
    llm_client: &Arc<Mutex<Box<dyn LlmClient + Send + Sync>>>,
) -> String {
    // Process the query using our query engine
    match query_engine.process_query(query).await {
        Ok(result) => match result {
            QueryResult::Summaries {
                summaries,
                timeframe,
                query,
                app_filter,
            } => {
                if let Some(app) = app_filter {
                    format_app_specific_summaries_with_ai(
                        summaries,
                        &timeframe.description,
                        &query,
                        &app,
                        llm_client,
                    )
                    .await
                } else {
                    format_summaries_with_ai(summaries, &timeframe.description, &query, llm_client)
                        .await
                }
            }
            QueryResult::Events {
                events,
                timeframe,
                query,
                app_filter,
            } => {
                if let Some(app) = app_filter {
                    format_app_specific_events_with_ai(
                        events,
                        &timeframe.description,
                        &query,
                        &app,
                        llm_client,
                    )
                    .await
                } else {
                    format_events_with_ai(events, &timeframe.description, &query, llm_client).await
                }
            }
            QueryResult::Empty {
                timeframe,
                app_filter,
                ..
            } => {
                if let Some(app) = app_filter {
                    format!(
                        "{}\n\nI don't have any data about what you did in {} during {}.",
                        random_fishy_no_data(),
                        app,
                        timeframe.description
                    )
                } else {
                    format!(
                        "{}\n\nI don't have any data about what you did during {}.",
                        random_fishy_no_data(),
                        timeframe.description
                    )
                }
            }
        },
        Err(e) => {
            eprintln!("Error processing query: {}", e);
            "🐠 Fishy looks confused... Something went wrong while I was searching my memory. Could you try asking in a different way?".to_string()
        }
    }
}

/// Format summaries using AI
async fn format_summaries_with_ai(
    summaries: Vec<activity_tracker_common::ActivitySummary>,
    timeframe: &str,
    query: &str,
    llm_client: &Arc<Mutex<Box<dyn LlmClient + Send + Sync>>>,
) -> String {
    if summaries.is_empty() {
        return random_fishy_no_data();
    }

    // Prepare the raw data for the LLM
    let raw_data = prepare_summaries_for_llm(&summaries);

    // Generate the AI response
    match generate_ai_response(&raw_data, timeframe, query, llm_client).await {
        Ok(ai_response) => {
            format!("{}{}", random_fishy_intro(), ai_response)
        }
        Err(e) => {
            eprintln!("Error generating AI response: {}", e);
            // Fallback to a simple format if AI fails
            let simple_response = format_summaries_simple(&summaries, timeframe);
            format!("{}{}", random_fishy_intro(), simple_response)
        }
    }
}

/// Format events using AI
async fn format_events_with_ai(
    events: Vec<UserEvent>,
    timeframe: &str,
    query: &str,
    llm_client: &Arc<Mutex<Box<dyn LlmClient + Send + Sync>>>,
) -> String {
    if events.is_empty() {
        return random_fishy_no_data();
    }

    // Prepare the raw data for the LLM
    let raw_data = prepare_events_for_llm(&events);

    // Generate the AI response
    match generate_ai_response(&raw_data, timeframe, query, llm_client).await {
        Ok(ai_response) => {
            format!("{}{}", random_fishy_intro(), ai_response)
        }
        Err(e) => {
            eprintln!("Error generating AI response: {}", e);
            // Fallback to a simple format if AI fails
            let simple_response = format_events_simple(&events, timeframe);
            format!("{}{}", random_fishy_intro(), simple_response)
        }
    }
}

/// Format summaries focusing on a specific app
async fn format_app_specific_summaries_with_ai(
    summaries: Vec<activity_tracker_common::ActivitySummary>,
    timeframe: &str,
    query: &str,
    app_name: &str,
    llm_client: &Arc<Mutex<Box<dyn LlmClient + Send + Sync>>>,
) -> String {
    if summaries.is_empty() {
        return format!(
            "{}\n\nI don't have any data about what you did in {} during {}.",
            random_fishy_no_data(),
            app_name,
            timeframe
        );
    }

    // Prepare the raw data for the LLM
    let raw_data = prepare_summaries_for_llm(&summaries);

    // Generate the app-specific AI response
    match generate_app_specific_response(&raw_data, timeframe, query, app_name, llm_client).await {
        Ok(ai_response) => {
            format!("{}{}", random_fishy_intro(), ai_response)
        }
        Err(e) => {
            eprintln!("Error generating app-specific AI response: {}", e);
            // Fallback to a simple format if AI fails
            let simple_response = format_app_summaries_simple(&summaries, timeframe, app_name);
            format!("{}{}", random_fishy_intro(), simple_response)
        }
    }
}

/// Format events focusing on a specific app
async fn format_app_specific_events_with_ai(
    events: Vec<UserEvent>,
    timeframe: &str,
    query: &str,
    app_name: &str,
    llm_client: &Arc<Mutex<Box<dyn LlmClient + Send + Sync>>>,
) -> String {
    if events.is_empty() {
        return format!(
            "{}\n\nI don't have any data about what you did in {} during {}.",
            random_fishy_no_data(),
            app_name,
            timeframe
        );
    }

    // Filter events to only include those from the specified app
    let app_name_lower = app_name.to_lowercase();
    let app_events: Vec<UserEvent> = events
        .into_iter()
        .filter(|event| {
            event
                .app_context
                .app_name
                .to_lowercase()
                .contains(&app_name_lower)
        })
        .collect();

    if app_events.is_empty() {
        return format!(
            "{}\n\nI don't have any data about what you did in {} during {}.",
            random_fishy_no_data(),
            app_name,
            timeframe
        );
    }

    // Prepare the raw data for the LLM
    let raw_data = prepare_events_for_llm(&app_events);

    // Generate the app-specific AI response
    match generate_app_specific_response(&raw_data, timeframe, query, app_name, llm_client).await {
        Ok(ai_response) => {
            format!("{}{}", random_fishy_intro(), ai_response)
        }
        Err(e) => {
            eprintln!("Error generating app-specific AI response: {}", e);
            // Fallback to a simple format if AI fails
            let simple_response = format_app_events_simple(&app_events, timeframe, app_name);
            format!("{}{}", random_fishy_intro(), simple_response)
        }
    }
}

/// Prepare summaries data for LLM processing
fn prepare_summaries_for_llm(summaries: &[activity_tracker_common::ActivitySummary]) -> String {
    let mut result = String::new();

    for summary in summaries {
        // Format time range
        let time_str = format!(
            "{} to {}",
            summary.start_time.format("%H:%M"),
            summary.end_time.format("%H:%M")
        );

        // Count apps
        let mut app_counts: HashMap<String, usize> = HashMap::new();
        for event in &summary.events {
            let app_name = &event.app_context.app_name;
            *app_counts.entry(app_name.clone()).or_insert(0) += 1;
        }

        // Get top apps
        let mut app_list: Vec<_> = app_counts.into_iter().collect();
        app_list.sort_by(|a, b| b.1.cmp(&a.1));
        let top_apps: Vec<_> = app_list
            .iter()
            .take(3)
            .map(|(name, count)| format!("{} ({} events)", name, count))
            .collect();

        // Add summary to result
        result.push_str(&format!("## Activity Period: {}\n\n", time_str));

        result.push_str(&format!("**Description**: {}\n\n", summary.description));

        result.push_str(&format!(
            "**Top Applications**: {}\n\n",
            top_apps.join(", ")
        ));

        result.push_str(&format!("**Tags**: {}\n\n", summary.tags.join(", ")));

        result.push_str(&format!(
            "**Event Count**: {} events\n\n",
            summary.events.len()
        ));

        result.push_str("---\n\n");
    }

    result
}

/// Prepare events data for LLM processing
fn prepare_events_for_llm(events: &[UserEvent]) -> String {
    if events.is_empty() {
        return "No events found.".to_string();
    }

    let mut result = String::new();

    // Time range
    let start_time = events
        .iter()
        .map(|e| e.timestamp)
        .min()
        .unwrap_or(Utc::now());
    let end_time = events
        .iter()
        .map(|e| e.timestamp)
        .max()
        .unwrap_or(Utc::now());

    result.push_str(&format!(
        "## Time Range: {} to {}\n\n",
        start_time.format("%H:%M"),
        end_time.format("%H:%M")
    ));

    // Count apps
    let mut app_counts: HashMap<String, usize> = HashMap::new();
    for event in events {
        let app_name = &event.app_context.app_name;
        *app_counts.entry(app_name.clone()).or_insert(0) += 1;
    }

    // Get top apps
    let mut app_list: Vec<_> = app_counts.into_iter().collect();
    app_list.sort_by(|a, b| b.1.cmp(&a.1));

    result.push_str("## Application Usage\n\n");
    result.push_str("| Application | Event Count |\n");
    result.push_str("|-------------|-------------|\n");

    for (app, count) in app_list.iter().take(5) {
        result.push_str(&format!("| {} | {} |\n", app, count));
    }

    result.push_str("\n");

    // Count event types
    let mut event_type_counts: HashMap<String, usize> = HashMap::new();
    for event in events {
        *event_type_counts.entry(event.event.clone()).or_insert(0) += 1;
    }

    result.push_str("## Event Types\n\n");
    result.push_str("| Event Type | Count |\n");
    result.push_str("|------------|-------|\n");

    for (event_type, count) in event_type_counts.iter() {
        result.push_str(&format!("| {} | {} |\n", event_type, count));
    }

    result.push_str("\n");

    // Sample of unique window titles
    let mut window_titles: HashSet<String> = HashSet::new();
    for event in events.iter().take(100) {
        // Limit to first 100 events
        window_titles.insert(event.app_context.window_title.clone());
    }

    if !window_titles.is_empty() {
        result.push_str("## Sample Window Titles\n\n");
        for title in window_titles.iter().take(5) {
            // Limit to 5 titles
            result.push_str(&format!("- {}\n", title));
        }
    }

    result
}

/// Generate an AI response based on the data
async fn generate_ai_response(
    raw_data: &str,
    timeframe: &str,
    query: &str,
    llm_client: &Arc<Mutex<Box<dyn LlmClient + Send + Sync>>>,
) -> Result<String, Box<dyn Error + Send + Sync>> {
    let prompt = format!(
        "You are Fishy, a helpful second brain assistant with a fun, aquatic personality. 
        Create a concise, informative response to the user's query about their activities during {}.
        
        USER QUERY: \"{}\"
        
        Here is the raw activity data:
        
        {}
        
        Please format your response as follows:
        1. A brief summary of the user's activity 
        2. A markdown table with each app's most relevent activity
        3. 1-2 brief insights or patterns you notice (optional)
        
        add ASCII art.
        Make the markdown table neat and properly formatted with column headers.
        DO NOT start with \"Fishy says\" as that will be added separately.
        ALWAYS address the user as 'you', not 'user'
        DO NOT use any prefix like 'Here's what I found:' - just start directly with the summary.",
        timeframe, query, raw_data
    );

    println!("Generating AI response, here's the input:\n\n{}", prompt);

    // Get a lock on the LLM client
    let client = llm_client.lock().await;

    // Generate the response
    let response = client.generate_text(&prompt).await?;

    Ok(response)
}

/// Simple formatter for summaries (fallback if AI fails)
fn format_summaries_simple(
    summaries: &[activity_tracker_common::ActivitySummary],
    timeframe: &str,
) -> String {
    let mut result = format!("Here's what I found for {}:\n\n", timeframe);

    for (i, summary) in summaries.iter().enumerate() {
        let time_range = format!(
            "{} to {}",
            summary.start_time.format("%H:%M"),
            summary.end_time.format("%H:%M")
        );

        result.push_str(&format!(
            "{}. {} - {} ({} events)\n",
            i + 1,
            time_range,
            summary.description.lines().next().unwrap_or("Activity"),
            summary.events.len()
        ));
    }

    result
}

/// Simple formatter for events (fallback if AI fails)
fn format_events_simple(events: &[UserEvent], timeframe: &str) -> String {
    let mut result = format!("Here's what I found for {}:\n\n", timeframe);

    // Count apps
    let mut app_counts: HashMap<String, usize> = HashMap::new();
    for event in events {
        let app_name = &event.app_context.app_name;
        *app_counts.entry(app_name.clone()).or_insert(0) += 1;
    }

    // Get top apps
    let mut app_list: Vec<_> = app_counts.into_iter().collect();
    app_list.sort_by(|a, b| b.1.cmp(&a.1));

    result.push_str("Top applications used:\n");
    for (i, (app, count)) in app_list.iter().take(3).enumerate() {
        result.push_str(&format!("{}. {} ({} events)\n", i + 1, app, count));
    }

    result.push_str(&format!("\nTotal events found: {}", events.len()));

    result
}

/// Generate an app-specific AI response based on the data
async fn generate_app_specific_response(
    raw_data: &str,
    timeframe: &str,
    query: &str,
    app_name: &str,
    llm_client: &Arc<Mutex<Box<dyn LlmClient + Send + Sync>>>,
) -> Result<String, Box<dyn Error + Send + Sync>> {
    let prompt = format!(
        "You are Fishy, a helpful second brain assistant with a fun, aquatic personality. 
        Create a concise, informative response to the user's query about their activities in {} during {}.
        
        USER QUERY: \"{}\"
        
        Here is the raw activity data:
        
        {}
        
        Please format your response as follows:
        1. A brief 1-2 sentence natural summary of what the user was doing specifically in {}
        2. A markdown table with the most relevant information focusing on their {} usage
        3. 1-2 brief insights or patterns you notice about how they used {} (optional)
        
        Keep your entire response under 15 lines for readability.
        Make the markdown table neat and properly formatted with column headers.
        DO NOT start with \"Fishy says\" as that will be added separately.
        DO NOT add excessive newlines or ASCII art.
        DO NOT use any prefix like 'Here's what I found:' - just start directly with the summary.",
        app_name, timeframe, query, raw_data, app_name, app_name, app_name
    );

    // Get a lock on the LLM client
    let client = llm_client.lock().await;

    // Generate the response
    let response = client.generate_text(&prompt).await?;

    Ok(response)
}

/// Simple formatter for app-specific summaries (fallback if AI fails)
fn format_app_summaries_simple(
    summaries: &[activity_tracker_common::ActivitySummary],
    timeframe: &str,
    app_name: &str,
) -> String {
    let mut result = format!(
        "Here's what I found about your {} usage during {}:\n\n",
        app_name, timeframe
    );

    for (i, summary) in summaries.iter().enumerate() {
        let time_range = format!(
            "{} to {}",
            summary.start_time.format("%H:%M"),
            summary.end_time.format("%H:%M")
        );

        result.push_str(&format!(
            "{}. {} - {} ({} events)\n",
            i + 1,
            time_range,
            summary.description.lines().next().unwrap_or("Activity"),
            summary.events.len()
        ));
    }

    result
}

/// Simple formatter for app-specific events (fallback if AI fails)
fn format_app_events_simple(events: &[UserEvent], timeframe: &str, app_name: &str) -> String {
    let mut result = format!(
        "Here's what I found about your {} usage during {}:\n\n",
        app_name, timeframe
    );

    // Count event types
    let mut event_type_counts: HashMap<String, usize> = HashMap::new();
    for event in events {
        *event_type_counts.entry(event.event.clone()).or_insert(0) += 1;
    }

    result.push_str(&format!(
        "You had {} total events in {}\n\n",
        events.len(),
        app_name
    ));

    // List event types
    result.push_str("Event types:\n");
    for (event_type, count) in event_type_counts.iter() {
        result.push_str(&format!("- {}: {} events\n", event_type, count));
    }

    result
}
