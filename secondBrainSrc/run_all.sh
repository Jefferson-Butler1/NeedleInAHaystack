#!/bin/bash

# Color codes for better output
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo -e "${GREEN}===== Starting Second Brain System =====${NC}"

# Starting directory
cd "$(dirname "$0")"

# Start the Docker services if they're not already running
echo -e "${BLUE}Starting database services...${NC}"
if ! docker ps | grep -q second-brain-timescaledb; then
    docker-compose up -d
    echo -e "${GREEN}Database services started${NC}"
else
    echo -e "${GREEN}Database services already running${NC}"
fi

# Check if Ollama is installed and running
if ! command -v ollama &> /dev/null; then
    echo -e "${YELLOW}Warning: Ollama not found. The Thinker module will not work correctly.${NC}"
    echo -e "${YELLOW}Install Ollama from https://ollama.ai/download${NC}"
else
    # Check if the model exists, if not download it
    if ! ollama list | grep -q "llama3.2:3b"; then
        echo -e "${BLUE}Downloading LLM model (this may take a while)...${NC}"
        ollama pull llama3.2:3b
    fi
fi

# Create a log directory
mkdir -p logs

# Initialize the SQLite database if needed
echo -e "${BLUE}Initializing SQLite database...${NC}"
./init_sqlite.sh

# Function to start a component
start_component() {
    local component="$1"
    local log_file="logs/${component}.log"
    
    echo -e "${BLUE}Starting ${component}...${NC}"
    cargo run -p activity-tracker-${component} > "$log_file" 2>&1 &
    echo $! > "logs/${component}.pid"
    echo -e "${GREEN}${component^} started (PID: $(cat "logs/${component}.pid"))${NC}"
}

# Start all components
start_component "learner"
start_component "thinker"
start_component "recall"

echo -e "${GREEN}===== All components started =====${NC}"
echo -e "${BLUE}To query the system:${NC} echo \"What was I working on today?\" | nc localhost 8080"
echo -e "${BLUE}To stop all components:${NC} ./stop_all.sh"
echo -e "${BLUE}Check logs in:${NC} ./logs/"