#!/bin/bash
set -e

echo "===== Second Brain Setup Script ====="

# Check if Cargo is installed
if ! command -v cargo &> /dev/null; then
    echo "Error: Rust and Cargo are required but not installed."
    echo "Please visit https://rustup.rs/ to install Rust."
    exit 1
fi

# Check if Ollama is installed
if ! command -v ollama &> /dev/null; then
    echo "Warning: Ollama is not installed or not in PATH."
    echo "The LLM features will not work without Ollama running llama3.2:3b model."
    echo "Please visit https://ollama.ai/ to install Ollama."
fi

# Check if PostgreSQL is running
if ! command -v pg_isready &> /dev/null; then
    echo "Warning: PostgreSQL CLI tools not found. Cannot check if PostgreSQL is running."
    echo "Please make sure PostgreSQL with TimescaleDB extension is installed and running."
else
    if ! pg_isready &> /dev/null; then
        echo "Warning: PostgreSQL is not running. Please start PostgreSQL."
    else
        echo "PostgreSQL is running."
    fi
fi

# Build the project
echo "Building Second Brain..."
cargo build

echo ""
echo "===== Setup Complete ====="
echo ""
echo "To run the Second Brain service:"
echo "  cargo run -- start"
echo ""
echo "To query your Second Brain:"
echo "  cargo run -- query \"What was I working on yesterday?\""
echo ""
echo "Make sure PostgreSQL with TimescaleDB is running, and Ollama"
echo "is running with the llama3.2:3b model pulled:"
echo "  ollama pull llama3.2:3b"
echo ""