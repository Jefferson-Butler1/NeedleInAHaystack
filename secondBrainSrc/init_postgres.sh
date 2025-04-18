#!/bin/bash

# Load environment variables
if [ -f .env ]; then
  source .env
else
  echo "No .env file found. Using default connection string."
  export DATABASE_URL="postgres://postgres:postgres@localhost:5438/second_brain"
fi

echo "Initializing PostgreSQL database at: $DATABASE_URL"

# Extract the port from the URL
port=$(echo $DATABASE_URL | sed -n 's/.*localhost:\([0-9]*\).*/\1/p')
echo "Using port: $port"

# Apply schema
PGPASSWORD=postgres psql -h localhost -p $port -U postgres -d second_brain -f schema.sql

echo "Database initialization complete."