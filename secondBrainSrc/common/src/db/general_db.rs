use crate::models::ActivitySummary;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::{Pool, Row, Sqlite, SqlitePool};
use std::error::Error;
use std::path::Path;

#[async_trait]
pub trait SummaryStore {
    async fn store_summary(&self, summary: &ActivitySummary) -> Result<(), Box<dyn Error>>;
    async fn get_summaries_in_timeframe(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Vec<ActivitySummary>, Box<dyn Error>>;
    async fn search_summaries(&self, query: &str) -> Result<Vec<ActivitySummary>, Box<dyn Error>>;
}

#[derive(Clone)]
pub struct GeneralDbClient {
    pool: Pool<Sqlite>,
}

impl GeneralDbClient {
    pub async fn new(connection_string: &str) -> Result<Self, Box<dyn Error>> {
        // If the connection string is a file path, ensure the directory exists
        if connection_string.starts_with("sqlite:") {
            let path = connection_string.trim_start_matches("sqlite:");
            if path != ":memory:" && !path.is_empty() {
                let db_path = Path::new(path);
                if let Some(parent) = db_path.parent() {
                    std::fs::create_dir_all(parent)?;
                }
            }
        }

        println!("Connecting to SQLite database: {}", connection_string);
        let pool = SqlitePool::connect(connection_string).await?;
        
        let client = Self { pool };
        
        // Ensure the required tables and indices exist
        client.ensure_schema().await?;
        
        Ok(client)
    }
    
    async fn ensure_schema(&self) -> Result<(), Box<dyn Error>> {
        // Create tables if they don't exist
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS activity_summaries (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                start_time TIMESTAMP NOT NULL,
                end_time TIMESTAMP NOT NULL,
                description TEXT NOT NULL,
                tags TEXT NOT NULL,
                events_json TEXT NOT NULL,
                created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
            );
            
            CREATE INDEX IF NOT EXISTS idx_summaries_time_range 
            ON activity_summaries(start_time, end_time);
            
            CREATE VIRTUAL TABLE IF NOT EXISTS summary_search 
            USING fts5(description, tags);
            "#
        )
        .execute(&self.pool)
        .await?;
        
        Ok(())
    }
    
    // Helper to convert between DB representation and ActivitySummary
    fn parse_summary_from_row(
        _id: i64,  // We don't use the ID in our ActivitySummary model, but it's useful for debugging
        start_time: DateTime<Utc>,
        end_time: DateTime<Utc>,
        description: String,
        tags_json: String,
        events_json: String,
    ) -> Result<ActivitySummary, Box<dyn Error>> {
        let tags: Vec<String> = serde_json::from_str(&tags_json)?;
        let events = serde_json::from_str(&events_json)?;
        
        Ok(ActivitySummary {
            start_time,
            end_time,
            description,
            events,
            tags,
        })
    }
}

#[async_trait]
impl SummaryStore for GeneralDbClient {
    async fn store_summary(&self, summary: &ActivitySummary) -> Result<(), Box<dyn Error>> {
        // Convert summary to DB representation
        let tags_json = serde_json::to_string(&summary.tags)?;
        let events_json = serde_json::to_string(&summary.events)?;
        
        // Start a transaction
        let mut tx = self.pool.begin().await?;
        
        // Insert into main table
        let summary_id = sqlx::query(
            r#"
            INSERT INTO activity_summaries
                (start_time, end_time, description, tags, events_json)
            VALUES (?, ?, ?, ?, ?)
            RETURNING id
            "#
        )
        .bind(summary.start_time)
        .bind(summary.end_time)
        .bind(&summary.description)
        .bind(&tags_json)
        .bind(&events_json)
        .fetch_one(&mut *tx)
        .await?
        .get::<i64, _>("id");
        
        // Insert into search index
        sqlx::query(
            r#"
            INSERT INTO summary_search 
                (rowid, description, tags)
            VALUES (?, ?, ?)
            "#
        )
        .bind(summary_id)
        .bind(&summary.description)
        .bind(&summary.tags.join(" "))
        .execute(&mut *tx)
        .await?;
        
        tx.commit().await?;
        
        Ok(())
    }

    async fn get_summaries_in_timeframe(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Vec<ActivitySummary>, Box<dyn Error>> {
        let rows = sqlx::query(
            r#"
            SELECT id, start_time, end_time, description, tags, events_json
            FROM activity_summaries
            WHERE 
                (start_time BETWEEN ? AND ?) OR
                (end_time BETWEEN ? AND ?) OR
                (start_time <= ? AND end_time >= ?)
            ORDER BY start_time DESC
            "#
        )
        .bind(start)
        .bind(end)
        .bind(start)
        .bind(end)
        .bind(start)
        .bind(end)
        .fetch_all(&self.pool)
        .await?;
        
        let mut summaries = Vec::with_capacity(rows.len());
        
        for row in rows {
            let id: i64 = row.get("id");
            let start_time: DateTime<Utc> = row.get("start_time");
            let end_time: DateTime<Utc> = row.get("end_time");
            let description: String = row.get("description");
            let tags_json: String = row.get("tags");
            let events_json: String = row.get("events_json");
            
            let summary = Self::parse_summary_from_row(
                id, start_time, end_time, description, tags_json, events_json
            )?;
            
            summaries.push(summary);
        }
        
        Ok(summaries)
    }

    async fn search_summaries(&self, query: &str) -> Result<Vec<ActivitySummary>, Box<dyn Error>> {
        // Format query for FTS5
        let search_query = format!("{}*", query.trim());
        
        let rows = sqlx::query(
            r#"
            SELECT a.id, a.start_time, a.end_time, a.description, a.tags, a.events_json
            FROM summary_search s
            JOIN activity_summaries a ON s.rowid = a.id
            WHERE s.description MATCH ? OR s.tags MATCH ?
            ORDER BY a.start_time DESC
            "#
        )
        .bind(&search_query)
        .bind(&search_query)
        .fetch_all(&self.pool)
        .await?;
        
        let mut summaries = Vec::with_capacity(rows.len());
        
        for row in rows {
            let id: i64 = row.get("id");
            let start_time: DateTime<Utc> = row.get("start_time");
            let end_time: DateTime<Utc> = row.get("end_time");
            let description: String = row.get("description");
            let tags_json: String = row.get("tags");
            let events_json: String = row.get("events_json");
            
            let summary = Self::parse_summary_from_row(
                id, start_time, end_time, description, tags_json, events_json
            )?;
            
            summaries.push(summary);
        }
        
        Ok(summaries)
    }
}
