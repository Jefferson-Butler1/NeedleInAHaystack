use crate::models::ActivitySummary;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::{Pool, Row, Sqlite, SqlitePool};
use std::error::Error;
use std::path::Path;

#[async_trait]
pub trait SummaryStore {
    async fn store_summary(&self, summary: &ActivitySummary) -> Result<(), Box<dyn Error + Send + Sync>>;
    async fn get_summaries_in_timeframe(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Vec<ActivitySummary>, Box<dyn Error + Send + Sync>>;
    async fn search_summaries(&self, query: &str) -> Result<Vec<ActivitySummary>, Box<dyn Error + Send + Sync>>;
}

#[derive(Clone)]
pub struct GeneralDbClient {
    pool: Pool<Sqlite>,
}

impl GeneralDbClient {
    pub async fn new(connection_string: &str) -> Result<Self, Box<dyn Error + Send + Sync>> {
        // If the connection string is a file path, ensure the directory exists
        if connection_string.starts_with("sqlite:") {
            let path = connection_string.trim_start_matches("sqlite:");
            if path != ":memory:" && !path.is_empty() {
                println!("Using SQLite database at path: {}", path);
                
                let db_path = Path::new(path);
                
                // Check if the database file already exists
                if !db_path.exists() {
                    println!("Database file doesn't exist yet, will be created");
                    
                    // Ensure parent directory exists
                    if let Some(parent) = db_path.parent() {
                        if !parent.exists() {
                            println!("Creating parent directory: {:?}", parent);
                            std::fs::create_dir_all(parent)?;
                        }
                    }
                } else {
                    // Check if file is writable
                    if std::fs::metadata(db_path)?.permissions().readonly() {
                        return Err(format!("Database file is read-only: {}", path).into());
                    }
                }
            }
        }

        println!("Connecting to SQLite database: {}", connection_string);
        
        let pool = match SqlitePool::connect(connection_string).await {
            Ok(pool) => pool,
            Err(e) => {
                eprintln!("SQLite connection error: {}", e);
                
                if connection_string.starts_with("sqlite:") {
                    let path = connection_string.trim_start_matches("sqlite:");
                    if path != ":memory:" && !path.is_empty() {
                        let db_path = Path::new(path);
                        
                        if !db_path.exists() {
                            eprintln!("Database file doesn't exist: {}", path);
                        } else {
                            eprintln!("Database file exists but couldn't connect. Check permissions.");
                            
                            // Try to get more info about the file
                            match std::fs::metadata(db_path) {
                                Ok(metadata) => {
                                    eprintln!("File size: {} bytes", metadata.len());
                                    let permissions = metadata.permissions();
                                    eprintln!("File permissions: readonly={}", permissions.readonly());
                                },
                                Err(e) => {
                                    eprintln!("Couldn't get file metadata: {}", e);
                                }
                            }
                        }
                    }
                }
                
                return Err(e.into());
            }
        };
        
        let client = Self { pool };
        
        // Ensure the required tables and indices exist
        client.ensure_schema().await?;
        
        Ok(client)
    }
    
    async fn ensure_schema(&self) -> Result<(), Box<dyn Error + Send + Sync>> {
        // Create activity_summaries table if it doesn't exist
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
            "#
        )
        .execute(&self.pool)
        .await?;
        
        // Check if summary_search table exists for FTS
        let result = sqlx::query("SELECT name FROM sqlite_master WHERE type='table' AND name='summary_search'")
            .fetch_optional(&self.pool)
            .await?;
            
        if result.is_none() {
            // Create the virtual FTS table for searching
            sqlx::query(
                r#"
                CREATE VIRTUAL TABLE IF NOT EXISTS summary_search
                USING FTS5(description, tags);
                "#
            )
            .execute(&self.pool)
            .await?;
        }
        
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
    ) -> Result<ActivitySummary, Box<dyn Error + Send + Sync>> {
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
    async fn store_summary(&self, summary: &ActivitySummary) -> Result<(), Box<dyn Error + Send + Sync>> {
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
    ) -> Result<Vec<ActivitySummary>, Box<dyn Error + Send + Sync>> {
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

    async fn search_summaries(&self, query: &str) -> Result<Vec<ActivitySummary>, Box<dyn Error + Send + Sync>> {
        // Simple approach: search for each word with LIKE
        let search_terms = query.split_whitespace()
            .filter(|term| !term.is_empty())
            .map(|term| format!("%{}%", term))
            .collect::<Vec<_>>();
        
        // If no search terms, return recent summaries
        if search_terms.is_empty() {
            let rows = sqlx::query(
                r#"
                SELECT id, start_time, end_time, description, tags, events_json
                FROM activity_summaries
                ORDER BY start_time DESC
                LIMIT 10
                "#
            )
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
            return Ok(summaries);
        }
        
        // Build a query that searches both description and tags using LIKE
        let mut combined_query = String::from(
            r#"
            SELECT id, start_time, end_time, description, tags, events_json
            FROM activity_summaries
            WHERE 
            "#
        );
        
        for (i, term) in search_terms.iter().enumerate() {
            if i > 0 {
                combined_query.push_str(" OR ");
            }
            combined_query.push_str(&format!("description LIKE '{}'", term));
            combined_query.push_str(&format!(" OR tags LIKE '{}'", term));
        }
        
        combined_query.push_str(" ORDER BY start_time DESC");
        
        let rows = sqlx::query(&combined_query)
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
            
            let summary = match Self::parse_summary_from_row(
                id, start_time, end_time, description, tags_json, events_json
            ) {
                Ok(s) => s,
                Err(e) => return Err(e),
            };
            
            summaries.push(summary);
        }
        
        Ok(summaries)
    }
}
