#!/bin/bash
set -e

echo "===== Second Brain Demo Setup ====="

# Check if Cargo is installed
if ! command -v cargo &> /dev/null; then
    echo "Error: Rust and Cargo are required but not installed."
    echo "Please visit https://rustup.rs/ to install Rust."
    exit 1
fi

# Check if Ollama is installed
if ! command -v ollama &> /dev/null; then
    echo "Warning: Ollama is not installed or not in PATH."
    echo "The LLM features will not work without Ollama running."
    echo "Please visit https://ollama.ai/ to install Ollama."
    OLLAMA_AVAILABLE=false
else
    OLLAMA_AVAILABLE=true
fi

# Pull the Ollama model if available
if [ "$OLLAMA_AVAILABLE" = true ]; then
    echo "Checking for llama3.2:3b model in Ollama..."
    if ! ollama list | grep -q "llama3.2:3b"; then
        echo "Pulling llama3.2:3b model (this might take a while)..."
        ollama pull llama3.2:3b
    else
        echo "Model llama3.2:3b is already available."
    fi
fi

# Build the demo
echo "Building Second Brain demo..."
cargo build --release --bin demo

echo ""
echo "===== Setup Complete ====="
echo ""
echo "To run the Second Brain demo service:"
echo "  ./target/release/demo start"
echo ""
echo "To query your Second Brain (demo mode):"
echo "  ./target/release/demo query \"What was I working on yesterday?\""
echo ""
echo "To test the LLM directly:"
echo "  ./target/release/demo test-llm \"Tell me a short joke\""
echo ""