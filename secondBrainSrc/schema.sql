-- Create user_summaries table if it doesn't exist
CREATE TABLE IF NOT EXISTS user_summaries (
    id SERIAL PRIMARY KEY,
    start_time TIMESTAMPTZ NOT NULL,
    end_time TIMESTAMPTZ NOT NULL,
    description TEXT NOT NULL,
    tags TEXT[] NOT NULL,
    keystrokes TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Create index for efficient time-based queries
CREATE INDEX IF NOT EXISTS user_summaries_timerange_idx ON user_summaries(start_time, end_time);

-- Create index for text search
CREATE INDEX IF NOT EXISTS user_summaries_description_idx ON user_summaries USING gin(to_tsvector('english', description));

-- Add test data if the table is empty
INSERT INTO user_summaries (start_time, end_time, description, tags, keystrokes, created_at)
SELECT 
    NOW() - INTERVAL '1 hour',
    NOW() - INTERVAL '30 minutes',
    'Working on the Second Brain project. Implemented the recall module with PostgreSQL support.',
    ARRAY['coding', 'rust', 'database'],
    'Typed code for PostgreSQL integration, including schema creation and query implementation.',
    NOW()
WHERE NOT EXISTS (SELECT 1 FROM user_summaries LIMIT 1);

INSERT INTO user_summaries (start_time, end_time, description, tags, keystrokes, created_at)
SELECT 
    NOW() - INTERVAL '3 hours',
    NOW() - INTERVAL '2 hours',
    'Working on the UI components. Building the TUI for better user interactions.',
    ARRAY['ui', 'tui', 'rust'],
    'Implemented TUI components using ratatui and crossterm libraries.',
    NOW()
WHERE NOT EXISTS (SELECT 1 FROM user_summaries LIMIT 1);

INSERT INTO user_summaries (start_time, end_time, description, tags, keystrokes, created_at)
SELECT 
    NOW() - INTERVAL '1 day',
    NOW() - INTERVAL '23 hours',
    'Morning coding session. Fixed bugs in the event tracking system.',
    ARRAY['debugging', 'morning', 'rust'],
    'Debugging keylogger module to improve window detection.',
    NOW()
WHERE NOT EXISTS (SELECT 1 FROM user_summaries LIMIT 1);