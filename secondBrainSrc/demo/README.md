# Second Brain Demo

This is a demonstration version of the Second Brain application, showing the concept without requiring a database setup.

## What is Second Brain?

Second Brain is an application that:
- Tracks your keystrokes, mouse clicks, and active applications
- Periodically summarizes your activities using an LLM
- Allows you to query your activity history in natural language

This demo version provides a taste of the query functionality using the Ollama LLM, but doesn't actually track or store activities.

## Requirements

- Rust 1.70+ (https://rustup.rs/)
- Ollama with the llama3.2:3b model (https://ollama.ai/)

## Setup

1. Install Ollama and run: `ollama pull llama3.2:3b`
2. Run the setup script: `./run_demo.sh`

## Usage

To start the demo service:
```
./target/release/second_brain_demo start
```

To query the demo (simulating what a full second brain would do):
```
./target/release/second_brain_demo query "What was I working on yesterday?"
```

To test the LLM directly:
```
./target/release/second_brain_demo test-llm "Tell me a short joke"
```

## Full Application

The full Second Brain application includes:
- A "learner" component that captures user activities
- A "thinker" component that processes and summarizes activities
- A "recall" component for searching and querying activity history
- PostgreSQL with TimescaleDB for efficient time-series storage

The full application is designed as a personal knowledge system that helps you remember what you've been working on and find information from your past activities.