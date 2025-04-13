mod llm;

use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;
use tokio::time::Duration;
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

use crate::llm::LlmClient;

#[derive(Parser)]
#[command(author, version, about = "Second Brain Demo", long_about = None)]
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
    },

    /// Query your second brain
    Query {
        /// The query string
        #[arg(required = true)]
        query: String,
    },

    /// Test the Ollama LLM integration
    TestLlm {
        /// The prompt to send to the LLM
        #[arg(required = true)]
        prompt: String,
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
        Some(Commands::Start { foreground }) => {
            info!("Starting second brain service (demo mode)");
            start_service().await?;
        }
        Some(Commands::Query { query }) => {
            info!("Processing query: {}", query);
            process_query(&query).await?;
        }
        Some(Commands::TestLlm { prompt }) => {
            info!("Testing LLM with prompt: {}", prompt);
            test_llm(&prompt).await?;
        }
        None => {
            info!("Starting second brain service in foreground (demo mode)");
            start_service().await?;
        }
    }

    Ok(())
}

async fn start_service() -> Result<()> {
    println!("Second Brain Demo Service");
    println!("-------------------------");
    println!("This demo version demonstrates the concept without a database.");
    println!("In the full version, the service would:");
    println!(" - Track your keystrokes, mouse clicks, and active applications");
    println!(" - Periodically summarize your activities using an LLM");
    println!(" - Allow you to query your history and activities");
    println!();
    println!("Press Ctrl+C to exit");

    // Keep the service running
    loop {
        tokio::time::sleep(Duration::from_secs(10)).await;
        info!("Service is still running...");
    }
}

async fn process_query(query_str: &str) -> Result<()> {
    info!("Processing query: {}", query_str);

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

async fn test_llm(prompt: &str) -> Result<()> {
    info!("Testing LLM with prompt: {}", prompt);

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

    println!("Sending prompt to Ollama: {}", prompt);

    match llm_client.generate(prompt).await {
        Ok(response) => {
            println!("\n--- Response from LLM ---");
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

