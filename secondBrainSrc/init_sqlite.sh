#!/bin/bash

# Make sure the database directory exists
mkdir -p /tmp/secondbrain_db
chmod 777 /tmp/secondbrain_db

# Only create the database if it doesn't exist
if [ ! -f "/tmp/secondbrain_db/summaries.db" ]; then
    echo "Creating SQLite database at /tmp/secondbrain_db/summaries.db"
    
    # Use SQLite to create the database and tables
    sqlite3 /tmp/secondbrain_db/summaries.db <<EOF
-- Create activity_summaries table
CREATE TABLE IF NOT EXISTS activity_summaries (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    start_time TEXT NOT NULL,
    end_time TEXT NOT NULL,
    description TEXT NOT NULL,
    tags TEXT NOT NULL,
    events_json TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- Create an index on timestamps for efficient queries
CREATE INDEX IF NOT EXISTS idx_summaries_time_range 
ON activity_summaries(start_time, end_time);

-- Create a virtual table for full-text search
CREATE VIRTUAL TABLE IF NOT EXISTS summary_search 
USING fts5(description, tags);
EOF
    
    echo "SQLite database created successfully"
    
    # Make sure the file is readable and writable
    chmod 666 /tmp/secondbrain_db/summaries.db
else
    echo "SQLite database already exists at /tmp/secondbrain_db/summaries.db"
    # Ensure permissions are correct
    chmod 666 /tmp/secondbrain_db/summaries.db
fi