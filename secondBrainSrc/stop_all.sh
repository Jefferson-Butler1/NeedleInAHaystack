#!/bin/bash

# Color codes for better output
GREEN='\033[0;32m'
RED='\033[0;31m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

echo -e "${RED}===== Stopping Second Brain System =====${NC}"

# Starting directory
cd "$(dirname "$0")"

# Function to stop a component
stop_component() {
    local component="$1"
    local pid_file="logs/${component}.pid"
    
    if [ -f "$pid_file" ]; then
        local pid=$(cat "$pid_file")
        echo -e "${BLUE}Stopping ${component} (PID: $pid)...${NC}"
        
        if ps -p "$pid" > /dev/null; then
            kill "$pid"
            echo -e "${GREEN}${component^} stopped${NC}"
        else
            echo -e "${RED}${component^} was not running${NC}"
        fi
        
        rm "$pid_file"
    else
        echo -e "${RED}${component^} was not running (no PID file found)${NC}"
    fi
}

# Stop all components
stop_component "learner"
stop_component "thinker"
stop_component "recall"

# Ask if Docker services should be stopped
read -p "Do you want to stop the database services? (y/n) " -n 1 -r
echo
if [[ $REPLY =~ ^[Yy]$ ]]; then
    echo -e "${BLUE}Stopping database services...${NC}"
    docker-compose down
    echo -e "${GREEN}Database services stopped${NC}"
fi

echo -e "${GREEN}===== All components stopped =====${NC}"