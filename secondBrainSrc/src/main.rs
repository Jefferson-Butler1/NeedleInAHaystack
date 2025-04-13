mod db;
mod learner;
mod models;
mod recall;
mod thinker;
mod utils;

use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;
use tokio::time::Duration;
use tracing::{info, warn, Level};
use tracing_subscriber::FmtSubscriber;

use crate::utils::llm::LlmClient;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// Path to configuration file
    #[arg(short, long, value_name = "FILE")]
    config: Option<PathBuf>,
}

#[derive(Subcommand)]
enum Commands {
    /// Start the second brain service
    Start {
        /// Run in foreground (don't daemonize)
        #[arg(short, long)]
        foreground: bool,

        /// Run in demo mode (no database required)
        #[arg(short, long)]
        demo: bool,
    },

    /// Query your second brain
    Query {
        /// The query string
        #[arg(required = true)]
        query: String,

        /// Run in demo mode (no database required)
        #[arg(short, long)]
        demo: bool,
    },

    /// Test the Ollama LLM integration
    TestLlm {
        /// The prompt to send to the LLM
        #[arg(required = true)]
        prompt: String,

        /// Specify a different model (default: llama3.2:3b)
        #[arg(short, long)]
        model: Option<String>,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    // Set up environment
    dotenv::dotenv().ok();

    // Set up logging
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;

    // Parse command line arguments
    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Start { foreground, demo }) => {
            if demo {
                info!("Starting second brain service in demo mode (no database)");
                start_service_demo().await?;
            } else {
                info!("Starting second brain service");
                start_service().await?;
            }
        }
        Some(Commands::Query { query, demo }) => {
            info!("Processing query: {}", query);
            if demo {
                process_query_demo(&query).await?;
            } else {
                process_query(&query).await?;
            }
        }
        Some(Commands::TestLlm { prompt, model }) => {
            let model_name = model.unwrap_or_else(|| "llama3.2:3b".to_string());
            info!(
                "Testing LLM with prompt using model {}: {}",
                model_name, prompt
            );
            test_llm(&prompt, &model_name).await?;
        }
        None => {
            info!("Starting second brain service in demo mode (no database)");
            start_service_demo().await?;
        }
    }

    Ok(())
}

async fn start_service() -> Result<()> {
    // Initialize database
    let db_pool = match db::init_db().await {
        Ok(pool) => pool,
        Err(e) => {
            // println!("Failed to connect to database: {}. Running in memory-only mode.", e);
            println!(
                "Error: Database connection failed. Please set up PostgreSQL with TimescaleDB."
            );
            println!("You can run in demo mode with: ./second_brain start --demo");
            return Err(anyhow::anyhow!("Database connection failed"));
        }
    };

    // Create LLM client
    let llm_client = match LlmClient::new() {
        Ok(client) => client,
        Err(e) => {
            println!("Failed to initialize LLM client: {}", e);
            println!("Error: LLM client initialization failed. Please make sure Ollama is running with llama3.2:3b model.");
            println!("You can pull it with: ollama pull llama3.2:3b");
            return Err(anyhow::anyhow!("LLM client initialization failed"));
        }
    };

    // Create event channel
    let (event_sender, mut event_receiver) = tokio::sync::mpsc::channel(100);

    // Create components
    let learner = learner::Learner::new(db_pool.clone(), event_sender);
    let thinker = thinker::Thinker::new(db_pool.clone(), llm_client.clone(), 15); // Process every 15 minutes

    // Start components in separate tasks
    tokio::spawn(async move {
        if let Err(e) = learner.start().await {
            println!("Learner error: {}", e);
        }
    });

    tokio::spawn(async move {
        if let Err(e) = thinker.start().await {
            println!("Thinker error: {}", e);
        }
    });

    // Process incoming events
    info!("Listening for events...");
    println!("Second Brain service started. Press Ctrl+C to exit.");

    while let Some(event) = event_receiver.recv().await {
        match db::timescale::insert_event(&db_pool, &event).await {
            Ok(_) => info!("Event stored successfully"),
            Err(e) => println!("Failed to store event: {}", e),
        }
    }

    Ok(())
}

async fn start_service_demo() -> Result<()> {
    info!("Second brain service started in demo mode (no database)");

    println!("Second Brain Demo Service");
    println!("-------------------------");
    println!("This demo version demonstrates the concept without requiring a database.");
    println!("In the full version, the service would:");
    println!(" - Track your keystrokes, mouse clicks, and active applications");
    println!(" - Periodically summarize your activities using an LLM");
    println!(" - Allow you to query your history and activities");
    println!();
    println!("Press Ctrl+C to exit");

    // Keep the service running
    loop {
        tokio::time::sleep(Duration::from_secs(60)).await;
        info!("Service is still running in demo mode...");
    }
}

async fn process_query(query_str: &str) -> Result<()> {
    // Initialize database
    let db_pool = match db::init_db().await {
        Ok(pool) => pool,
        Err(e) => {
            println!("Failed to connect to database: {}", e);
            println!(
                "Error: Database connection failed. Please set up PostgreSQL with TimescaleDB."
            );
            println!(
                "You can run in demo mode with: ./second_brain query --demo \"{}\"",
                query_str
            );
            return Err(anyhow::anyhow!("Database connection failed"));
        }
    };

    // Create LLM client
    let llm_client = match LlmClient::new() {
        Ok(client) => client,
        Err(e) => {
            println!("Failed to initialize LLM client: {}", e);
            println!("Error: LLM client initialization failed. Please make sure Ollama is running with llama3.2:3b model.");
            println!("You can pull it with: ollama pull llama3.2:3b");
            return Err(anyhow::anyhow!("LLM client initialization failed"));
        }
    };

    // Create recall component
    let recall = recall::Recall::new(db_pool, llm_client);

    // Process the query
    println!("Processing query: \"{}\"", query_str);

    let results = match recall.natural_language_query(query_str).await {
        Ok(res) => res,
        Err(e) => {
            println!("Query processing failed: {}", e);
            println!("Error: Failed to process your query. Please try again.");
            return Err(anyhow::anyhow!("Query processing failed"));
        }
    };

    if results.is_empty() {
        println!("No results found for your query.");
        return Ok(());
    }

    // Generate a meta-summary
    println!("Generating summary...");
    let summary = match recall.meta_summarize(&results).await {
        Ok(s) => s,
        Err(e) => {
            println!("Summary generation failed: {}", e);
            "Unable to generate summary.".to_string()
        }
    };

    // Print results
    println!("\n--- Summary ---");
    println!("{}\n", summary);

    println!("--- Details ---");
    for (i, result) in results.iter().enumerate() {
        println!(
            "{}. [{}] {}",
            i + 1,
            result.start_time.format("%Y-%m-%d %H:%M"),
            result.description
        );
    }

    Ok(())
}

async fn process_query_demo(query_str: &str) -> Result<()> {
    info!("Processing query in demo mode: {}", query_str);

    // Create LLM client
    let llm_client = match LlmClient::new() {
        Ok(client) => client,
        Err(e) => {
            println!("Error: Could not initialize LLM client: {}", e);
            println!("Please make sure Ollama is running with llama3.2:3b model.");
            println!("You can pull it with: ollama pull llama3.2:3b");
            println!("Exiting due to LLM initialization error.");
            std::process::exit(1);
        }
    };

    // Demo query processing
    println!(
        "Processing query in demo mode (no database): \"{}\"",
        query_str
    );

    let prompt = format!(
        "You are part of a second brain application that helps users remember their activities. \
        The user has asked: \"{}\"\n\
        Although you don't have access to their actual activity data, \
        please provide a helpful response about how this type of query would be processed \
        in a fully working second brain system. Explain what data might be retrieved \
        and how it would be presented to them.",
        query_str
    );

    match llm_client.generate(&prompt).await {
        Ok(response) => {
            println!("\n--- Response ---");
            println!("{}", response);
        }
        Err(e) => {
            println!("Error: Failed to get response from LLM: {}", e);
            println!("Please make sure Ollama is running with llama3.2:3b model.");
            println!("Exiting due to LLM error.");
            std::process::exit(1);
        }
    }

    Ok(())
}

async fn test_llm(prompt: &str, model: &str) -> Result<()> {
    info!("Testing LLM with prompt: {}", prompt);

    // Create LLM client
    let llm_client = match if model == "llama3.2:3b" {
        LlmClient::new()
    } else {
        LlmClient::with_model(model)
    } {
        Ok(client) => client,
        Err(e) => {
            println!("Error: Could not initialize LLM client: {}", e);
            println!(
                "Please make sure Ollama is running with the {} model.",
                model
            );
            println!("You can pull it with: ollama pull {}", model);
            println!("Exiting due to LLM initialization error.");
            std::process::exit(1);
        }
    };

    println!("Sending prompt to Ollama model {}: {}", model, prompt);

    match llm_client.generate(prompt).await {
        Ok(response) => {
            println!("\n--- Response from LLM ---");
            println!("{}", response);
        }
        Err(e) => {
            println!("Error: Failed to get response from LLM: {}", e);
            println!(
                "Please make sure Ollama is running with the {} model.",
                model
            );
            println!("Exiting due to LLM error.");
            std::process::exit(1);
        }
    }

    Ok(())
}

