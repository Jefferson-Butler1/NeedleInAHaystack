#!/bin/bash
set -e

echo "===== Second Brain Database Setup ====="

# Check if PostgreSQL is installed and running
if ! command -v psql &> /dev/null; then
    echo "Error: PostgreSQL not found. Please install PostgreSQL first."
    exit 1
fi

# Check if PostgreSQL is running
if ! pg_isready &> /dev/null; then
    echo "Error: PostgreSQL is not running. Please start PostgreSQL first."
    exit 1
fi

# Create database
echo "Creating second_brain database..."
psql -U postgres -c "DROP DATABASE IF EXISTS second_brain;" || true
psql -U postgres -c "CREATE DATABASE second_brain;"

# Check for TimescaleDB extension
echo "Checking and installing TimescaleDB extension..."
if ! psql -U postgres -d second_brain -c "SELECT extname FROM pg_extension WHERE extname = 'timescaledb';" | grep -q timescaledb; then
    psql -U postgres -d second_brain -c "CREATE EXTENSION IF NOT EXISTS timescaledb CASCADE;"
    echo "TimescaleDB extension installed."
else
    echo "TimescaleDB extension already installed."
fi

# Apply migrations
echo "Applying database migrations..."
psql -U postgres -d second_brain -f migrations/20250410000000_initial_schema.sql

echo ""
echo "===== Database Setup Complete ====="
echo ""
echo "PostgreSQL database 'second_brain' has been created and initialized."
echo "Your application should now be able to connect to the database."
echo ""