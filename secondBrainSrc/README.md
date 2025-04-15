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
   - Uses Ollama (llama3.2:3b) to create descriptions of user behavior
   - Extracts keywords and stores in a searchable database

3. **Recall** - Retrieves and analyzes past activities:
   - Supports natural language queries about past activities
   - Enables fuzzy search for finding related activities
   - Creates meta-summaries across time periods

## Project Structure

The project is organized as a Cargo workspace with multiple crates:

```
second-brain/
├── Cargo.toml (workspace)
├── crates/
│   ├── common/ - Shared code and utilities
│   │   ├── src/
│   │   │   ├── db/ - Database interfaces
│   │   │   ├── llm/ - LLM clients
│   │   │   ├── models.rs - Shared data models
│   │   │   └── utils.rs - Utility functions
│   ├── learner/ - Activity tracking
│   ├── thinker/ - Processing and analyzing
│   └── recall/ - Retrieval and querying
└── docker-compose.yml - Services configuration
```

## Prerequisites

- Rust 1.70+
- Docker and Docker Compose

## Setup

### Clone the repository

```bash
git clone https://github.com/yourusername/second-brain.git
cd second-brain
```

### Configure environment

Copy the example environment file and edit as needed:

```bash
cp .env.sample .env
```

### Start the services

The system uses Docker Compose to manage:
- TimescaleDB for time-series data storage
- Ollama for LLM inference

```bash
docker-compose up -d
```

This will start:
- TimescaleDB on port 5435
- Ollama on port 11434 with the llama3.2:3b model

### Building the application

```bash
cargo build --release
```

## Usage

Start the service to begin capturing your activities:

```bash
./target/release/second-brain
```

This will launch all three components:
- The Learner will begin capturing keystrokes and activity
- The Thinker will process this data every 5 minutes
- The Recall service will listen on port 8080 for queries

### Querying your Second Brain

You can query your second brain through the TCP interface:

```bash
echo "What was I working on yesterday?" | nc localhost 8080
```

Or for fuzzy search:

```bash
echo "fuzzy:rust async" | nc localhost 8080
```

## Development

### Running the components individually

You can run each component separately during development:

```bash
# Start the learner
cargo run --package activity-tracker-learner

# Start the thinker
cargo run --package activity-tracker-thinker

# Start the recall service
cargo run --package activity-tracker-recall
```

### Accessing TimescaleDB directly

```bash
docker-compose exec timescaledb psql -U postgres -d second_brain
```

## License

MIT
