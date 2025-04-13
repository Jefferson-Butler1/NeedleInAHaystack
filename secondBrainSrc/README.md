# Second Brain

A personal knowledge capture and retrieval system that tracks your activities, learns from them, and helps you recall what you've done.

## Architecture

The system consists of three main components:

1. **Learner** - Captures user activities:
   - Intercepts keystrokes, mouse clicks
   - Tracks active applications
   - Stores data in a TimescaleDB time-series database

2. **Thinker** - Processes activity data:
   - Periodically summarizes raw activities
   - Uses an LLM to create descriptions of user behavior
   - Extracts keywords and stores in a searchable database

3. **Recall** - Retrieves and analyzes past activities:
   - Supports natural language queries about past activities
   - Enables fuzzy search for finding related activities
   - Creates meta-summaries across time periods

## Prerequisites

For the full version:
- Rust 1.70+
- PostgreSQL with TimescaleDB extension
- Ollama with llama3.2:3b model

For the demo version (no database required):
- Rust 1.70+
- Ollama with llama3.2:3b model

## Setup

### Installing Ollama

1. Download and install Ollama from [ollama.ai](https://ollama.ai)
2. Pull the llama3.2 model:
   ```
   ollama pull llama3.2:3b
   ```

### Setting up the full version

1. Install PostgreSQL and TimescaleDB
2. Set up the database:
   ```
   psql -U postgres -c "CREATE DATABASE second_brain;"
   psql -U postgres -d second_brain -c "CREATE EXTENSION IF NOT EXISTS timescaledb CASCADE;"
   ```
3. Run the database setup script:
   ```
   ./db_setup.sh
   ```
4. Copy `.env.sample` to `.env` and configure your database connection

### Building the application

```
cargo build --release
```

## Usage

### Demo Mode (no database required)

The demo mode lets you try the basic functionality without setting up a database:

```bash
# Start the demo service
./target/release/second_brain start --demo

# Query in demo mode
./target/release/second_brain query --demo "What was I working on yesterday?"

# Test the LLM directly
./target/release/second_brain test-llm "Tell me a short joke"

# Use a different model (if you have it pulled in Ollama)
./target/release/second_brain test-llm --model "llama3:70b" "Explain quantum computing"
```

### Full Mode (requires database setup)

Start the service to begin capturing your activities:

```
./target/release/second_brain start
```

Query your second brain:

```
./target/release/second_brain query "What was I working on last Tuesday?"
./target/release/second_brain query "Find my research on rust async"
```

## License

MIT