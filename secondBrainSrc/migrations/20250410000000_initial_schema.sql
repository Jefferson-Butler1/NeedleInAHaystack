-- Create the event_type enum
CREATE TYPE event_type AS ENUM ('keystroke', 'mouseclick', 'appswitch', 'screencapture');

-- Create user_events table for TimescaleDB
CREATE TABLE user_events (
    id BIGSERIAL,
    timestamp TIMESTAMPTZ NOT NULL,
    event_type event_type NOT NULL,
    data JSONB NOT NULL,
    app_name TEXT NOT NULL,
    PRIMARY KEY (id, timestamp)
);

-- Create hypertable for TimescaleDB (assuming TimescaleDB extension is enabled)
CREATE EXTENSION IF NOT EXISTS timescaledb CASCADE;
SELECT create_hypertable('user_events', 'timestamp');

-- Create table for activity summaries
CREATE TABLE activity_summaries (
    id BIGSERIAL PRIMARY KEY,
    start_time TIMESTAMPTZ NOT NULL,
    end_time TIMESTAMPTZ NOT NULL,
    description TEXT NOT NULL,
    apps_used TEXT[] NOT NULL,
    keywords TEXT[] NOT NULL
);

-- Create indexes for better query performance
CREATE INDEX idx_user_events_app_name ON user_events (app_name);
CREATE INDEX idx_activity_summaries_timerange ON activity_summaries (start_time, end_time);
CREATE INDEX idx_activity_summaries_keywords ON activity_summaries USING GIN (keywords);