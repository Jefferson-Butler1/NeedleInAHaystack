use crate::models::{AppContext, UserEvent};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::{Pool, Postgres, Row};
use std::error::Error;

mod general_db;
pub use general_db::*;

#[async_trait]
pub trait EventStore {
    async fn store_event(&self, event: UserEvent) -> Result<(), Box<dyn Error>>;
    async fn get_events_in_timeframe(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Vec<UserEvent>, Box<dyn Error>>;
}

#[async_trait]
pub trait TimescaleSummaryStore {
    async fn store_timescale_summary(
        &self, 
        start_time: DateTime<Utc>,
        end_time: DateTime<Utc>,
        description: String,
        tags: Vec<String>,
        keystrokes: String,
    ) -> Result<(), Box<dyn Error>>;
    
    async fn get_timescale_summaries_for_day(
        &self,
        day: DateTime<Utc>,
    ) -> Result<Vec<(DateTime<Utc>, DateTime<Utc>, String, Vec<String>)>, Box<dyn Error>>;
    
    async fn search_timescale_summaries(
        &self,
        query: &str,
    ) -> Result<Vec<(DateTime<Utc>, DateTime<Utc>, String, Vec<String>)>, Box<dyn Error>>;
}

#[derive(Clone)]
pub struct TimescaleClient {
    pool: Pool<Postgres>,
}

impl TimescaleClient {
    pub async fn new(connection_string: &str) -> Result<Self, Box<dyn Error>> {
        println!("Connecting to database: {}", connection_string);
        let pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(5)
            .connect(connection_string)
            .await?;
        
        let client = Self { pool };
        
        // First, check if we need to drop existing tables
        // This is temporary for development - remove in production
        if let Ok(tables) = sqlx::query("SELECT to_regclass('user_events') as exists")
            .fetch_one(&client.pool)
            .await
        {
            // If table exists but is wrong, drop it
            let exists: Option<String> = tables.try_get("exists").unwrap_or(None);
            if exists.is_some() {
                println!("Dropping existing user_events table to recreate it correctly");
                sqlx::query("DROP TABLE user_events CASCADE;")
                    .execute(&client.pool)
                    .await?;
            }
        }
        
        // Ensure the required tables exist
        client.ensure_tables_exist().await?;
        
        Ok(client)
    }
    
    // Create the necessary tables if they don't exist
    async fn ensure_tables_exist(&self) -> Result<(), Box<dyn Error>> {
        // Create the user_events table if it doesn't exist
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS user_events (
                id SERIAL PRIMARY KEY,
                timestamp TIMESTAMPTZ NOT NULL,
                event_type TEXT NOT NULL,
                event_data TEXT NOT NULL,
                app_name TEXT NOT NULL,
                window_title TEXT NOT NULL,
                url TEXT
            )
            "#
        )
        .execute(&self.pool)
        .await?;
        
        // Create user_summaries table for timestamped summaries
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS user_summaries (
                id SERIAL PRIMARY KEY,
                start_time TIMESTAMPTZ NOT NULL,
                end_time TIMESTAMPTZ NOT NULL,
                description TEXT NOT NULL,
                tags TEXT[] NOT NULL,
                keystrokes TEXT NOT NULL,
                created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
            )
            "#
        )
        .execute(&self.pool)
        .await?;
        
        // Create indices - one at a time (PostgreSQL doesn't allow multiple commands in a prepared statement)
        sqlx::query(
            r#"
            CREATE INDEX IF NOT EXISTS user_events_timestamp_idx ON user_events (timestamp)
            "#
        )
        .execute(&self.pool)
        .await?;
        
        sqlx::query(
            r#"
            CREATE INDEX IF NOT EXISTS user_summaries_timerange_idx ON user_summaries (start_time, end_time)
            "#
        )
        .execute(&self.pool)
        .await?;
        
        Ok(())
    }
}

#[async_trait]
impl EventStore for TimescaleClient {
    async fn store_event(&self, event: UserEvent) -> Result<(), Box<dyn Error>> {
        // First, check if the events table exists, create it if it doesn't
        self.ensure_tables_exist().await?;
        
        // Insert the event into the database
        sqlx::query(
            r#"
            INSERT INTO user_events (timestamp, event_type, event_data, app_name, window_title, url)
            VALUES ($1, $2, $3, $4, $5, $6)
            "#
        )
        .bind(event.timestamp)
        .bind(&event.event)
        .bind(&event.data)
        .bind(&event.app_context.app_name)
        .bind(&event.app_context.window_title)
        .bind(&event.app_context.url)
        .execute(&self.pool)
        .await?;
        
        Ok(())
    }
    
    async fn get_events_in_timeframe(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Vec<UserEvent>, Box<dyn Error>> {
        // Query events within the timeframe
        let rows = sqlx::query(
            r#"
            SELECT timestamp, event_type as "event_type!", event_data as "event_data!", 
                  app_name as "app_name!", window_title as "window_title!", url
            FROM user_events
            WHERE timestamp >= $1 AND timestamp <= $2
            ORDER BY timestamp ASC
            "#
        )
        .bind(start)
        .bind(end)
        .fetch_all(&self.pool)
        .await?;

        // Convert rows to UserEvent objects
        let mut events = Vec::with_capacity(rows.len());
        
        for row in rows {
            let timestamp: DateTime<Utc> = row.try_get("timestamp")?;
            let event_type: String = row.try_get("event_type!")?;
            let event_data: String = row.try_get("event_data!")?;
            let app_name: String = row.try_get("app_name!")?;
            let window_title: String = row.try_get("window_title!")?;
            let url: Option<String> = row.try_get("url").ok();
            
            events.push(UserEvent {
                timestamp,
                event: event_type,
                data: event_data,
                app_context: AppContext {
                    app_name,
                    window_title,
                    url,
                },
            });
        }

        Ok(events)
    }
}

#[async_trait]
impl TimescaleSummaryStore for TimescaleClient {
    async fn store_timescale_summary(
        &self,
        start_time: DateTime<Utc>,
        end_time: DateTime<Utc>,
        description: String,
        tags: Vec<String>,
        keystrokes: String,
    ) -> Result<(), Box<dyn Error>> {
        // Ensure tables exist
        self.ensure_tables_exist().await?;
        
        // Insert the summary into the database
        sqlx::query(
            r#"
            INSERT INTO user_summaries (start_time, end_time, description, tags, keystrokes)
            VALUES ($1, $2, $3, $4, $5)
            "#
        )
        .bind(start_time)
        .bind(end_time)
        .bind(&description)
        .bind(&tags)
        .bind(&keystrokes)
        .execute(&self.pool)
        .await?;
        
        Ok(())
    }
    
    async fn get_timescale_summaries_for_day(
        &self,
        day: DateTime<Utc>,
    ) -> Result<Vec<(DateTime<Utc>, DateTime<Utc>, String, Vec<String>)>, Box<dyn Error>> {
        // Calculate day start and end
        let day_start = day.date_naive().and_hms_opt(0, 0, 0).unwrap().and_utc();
        let day_end = day.date_naive().and_hms_opt(23, 59, 59).unwrap().and_utc();
        
        // Query summaries for the day
        let rows = sqlx::query(
            r#"
            SELECT start_time, end_time, description, tags
            FROM user_summaries
            WHERE 
                (start_time BETWEEN $1 AND $2) OR
                (end_time BETWEEN $1 AND $2) OR
                (start_time <= $1 AND end_time >= $2)
            ORDER BY start_time ASC
            "#
        )
        .bind(day_start)
        .bind(day_end)
        .fetch_all(&self.pool)
        .await?;
        
        // Convert rows to summary tuples
        let mut summaries = Vec::with_capacity(rows.len());
        
        for row in rows {
            let start_time: DateTime<Utc> = row.try_get("start_time")?;
            let end_time: DateTime<Utc> = row.try_get("end_time")?;
            let description: String = row.try_get("description")?;
            let tags: Vec<String> = row.try_get("tags")?;
            
            summaries.push((start_time, end_time, description, tags));
        }
        
        Ok(summaries)
    }
    
    async fn search_timescale_summaries(
        &self,
        query: &str,
    ) -> Result<Vec<(DateTime<Utc>, DateTime<Utc>, String, Vec<String>)>, Box<dyn Error>> {
        // Query summaries that match the search term in description or tags
        let rows = sqlx::query(
            r#"
            SELECT start_time, end_time, description, tags
            FROM user_summaries
            WHERE 
                description ILIKE $1 OR
                $1 = ANY(tags)
            ORDER BY start_time DESC
            LIMIT 50
            "#
        )
        .bind(format!("%{}%", query))
        .fetch_all(&self.pool)
        .await?;
        
        // Convert rows to summary tuples
        let mut summaries = Vec::with_capacity(rows.len());
        
        for row in rows {
            let start_time: DateTime<Utc> = row.try_get("start_time")?;
            let end_time: DateTime<Utc> = row.try_get("end_time")?;
            let description: String = row.try_get("description")?;
            let tags: Vec<String> = row.try_get("tags")?;
            
            summaries.push((start_time, end_time, description, tags));
        }
        
        Ok(summaries)
    }
}
