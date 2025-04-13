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
fi

# Build the demo
echo "Building Second Brain demo..."
cargo build --release

# Create a symlink in the parent directory
echo "Creating symlink in parent directory..."
cd ..
ln -sf demo/target/release/second_brain_demo second_brain_demo

echo ""
echo "===== Setup Complete ====="
echo ""
echo "To run the Second Brain demo service:"
echo "  ./second_brain_demo start"
echo ""
echo "To query your Second Brain (demo mode):"
echo "  ./second_brain_demo query \"What was I working on yesterday?\""
echo ""
echo "To test the LLM directly:"
echo "  ./second_brain_demo test-llm \"Tell me a short joke\""
echo ""
echo "Note: Please ensure Ollama is running with the llama3.2:3b model."
echo "You can pull it with: ollama pull llama3.2:3b"
echo ""