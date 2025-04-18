use activity_tracker_common::{
    ActivitySummary, UserEvent,
    db::{GeneralDbClient, EventStore, TimescaleClient}
};
use chrono::{DateTime, Datelike, Duration, Local, NaiveTime, TimeZone, Timelike, Utc};
use sqlx::{Pool, PgPool, Row, Postgres, postgres::PgRow};
use std::error::Error;
use std::sync::Arc;

/// QueryEngine is responsible for processing user queries and retrieving relevant data
#[derive(Clone)]
pub struct QueryEngine {
    pg_pool: Arc<Pool<Postgres>>,
    event_db: Option<Arc<TimescaleClient>>,
}

impl QueryEngine {
    pub fn new(pg_pool: Arc<Pool<Postgres>>, event_db: Option<Arc<TimescaleClient>>) -> Self {
        Self { 
            pg_pool,
            event_db
        }
    }

    /// Process a natural language query and return relevant summaries or events
    pub async fn process_query(&self, query: &str) -> Result<QueryResult, Box<dyn Error + Send + Sync>> {
        // Parse the timeframe from the query
        let timeframe = self.parse_timeframe(query);
        
        // Check if the query is app-specific
        let app_filter = self.extract_app_name(query);
        
        // Try to get summaries for the timeframe from PostgreSQL, optionally filtered by app
        let summaries = if let Some(app_name) = &app_filter {
            self.get_summaries_by_app(timeframe.start, timeframe.end, app_name).await?
        } else {
            self.get_summaries_in_timeframe(timeframe.start, timeframe.end).await?
        };
        
        if !summaries.is_empty() {
            // We found summaries, return them
            return Ok(QueryResult::Summaries {
                summaries,
                timeframe,
                query: query.to_string(),
                app_filter,
            });
        }
        
        // If no summaries and we have an event database, try to get events directly
        if let Some(event_db) = &self.event_db {
            let events = if let Some(app_name) = &app_filter {
                // Query events filtered by app name
                self.get_events_by_app(event_db, timeframe.start, timeframe.end, app_name).await?
            } else {
                event_db.get_events_in_timeframe(timeframe.start, timeframe.end).await?
            };
            
            if !events.is_empty() {
                return Ok(QueryResult::Events {
                    events,
                    timeframe,
                    query: query.to_string(),
                    app_filter,
                });
            }
        }
        
        // If we still don't have results, try to search summaries by content
        let clean_query = self.sanitize_query_for_search(query);
        let summaries = if let Some(app_name) = &app_filter {
            self.search_summaries_by_app(&clean_query, app_name).await?
        } else {
            self.search_summaries(&clean_query).await?
        };
        
        if !summaries.is_empty() {
            return Ok(QueryResult::Summaries {
                summaries,
                timeframe,
                query: query.to_string(),
                app_filter,
            });
        }
        
        // If we haven't found anything, return an empty result
        Ok(QueryResult::Empty {
            timeframe,
            query: query.to_string(),
            app_filter,
        })
    }
    
    /// Extract application name from the query if present
    fn extract_app_name(&self, query: &str) -> Option<String> {
        let query = query.to_lowercase();
        
        // Common apps to look for
        let known_apps = [
            "ghostty", "vscode", "visual studio code", "chrome", "firefox", 
            "safari", "terminal", "slack", "discord", "notion", "figma"
        ];
        
        // Check for "in {app}" or "using {app}" patterns
        for &app in &known_apps {
            let patterns = [
                format!(" in {} ", app),
                format!(" on {} ", app),
                format!(" using {} ", app),
                format!(" with {} ", app),
                format!(" at {} ", app),
            ];
            
            for pattern in &patterns {
                if query.contains(pattern) {
                    return Some(app.to_string());
                }
            }
            
            // Also check if app name is at the beginning or end of a sentence
            let start_patterns = [
                format!(" in {}.", app),
                format!(" on {}.", app),
                format!(" using {}.", app),
                format!(" with {}.", app),
                format!(" at {}.", app),
                format!(" in {}?", app),
                format!(" on {}?", app),
                format!(" using {}?", app),
                format!(" with {}?", app),
                format!(" at {}?", app),
            ];
            
            for pattern in &start_patterns {
                if query.contains(pattern) {
                    return Some(app.to_string());
                }
            }
        }
        
        None
    }
    
    /// Get events filtered by app name
    async fn get_events_by_app(
        &self,
        event_db: &Arc<TimescaleClient>,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        app_name: &str,
    ) -> Result<Vec<UserEvent>, Box<dyn Error + Send + Sync>> {
        // Get all events within the timeframe
        let events = event_db.get_events_in_timeframe(start, end).await?;
        
        // Filter events by app name
        let app_name_lower = app_name.to_lowercase();
        let filtered_events = events
            .into_iter()
            .filter(|event| event.app_context.app_name.to_lowercase().contains(&app_name_lower))
            .collect();
        
        Ok(filtered_events)
    }
    
    /// Get summaries within a specific timeframe from PostgreSQL
    async fn get_summaries_in_timeframe(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Vec<ActivitySummary>, Box<dyn Error + Send + Sync>> {
        // Query summaries from PostgreSQL user_summaries table
        let rows = sqlx::query(
            r#"
            SELECT 
                id, start_time, end_time, description, tags, 
                keystrokes, created_at
            FROM 
                user_summaries
            WHERE 
                (start_time BETWEEN $1 AND $2) OR
                (end_time BETWEEN $1 AND $2) OR
                (start_time <= $1 AND end_time >= $2)
            ORDER BY 
                start_time DESC
            "#
        )
        .bind(start)
        .bind(end)
        .fetch_all(&*self.pg_pool)
        .await?;
        
        let mut summaries = Vec::with_capacity(rows.len());
        
        for row in rows {
            let summary = self.parse_summary_from_row(row)?;
            summaries.push(summary);
        }
        
        Ok(summaries)
    }
    
    /// Search summaries by content in PostgreSQL
    async fn search_summaries(
        &self,
        search_term: &str,
    ) -> Result<Vec<ActivitySummary>, Box<dyn Error + Send + Sync>> {
        // Query summaries from PostgreSQL by searching description
        let search_pattern = format!("%{}%", search_term);
        
        let rows = sqlx::query(
            r#"
            SELECT 
                id, start_time, end_time, description, tags, 
                keystrokes, created_at
            FROM 
                user_summaries
            WHERE 
                description ILIKE $1
            ORDER BY 
                start_time DESC
            LIMIT 10
            "#
        )
        .bind(search_pattern)
        .fetch_all(&*self.pg_pool)
        .await?;
        
        let mut summaries = Vec::with_capacity(rows.len());
        
        for row in rows {
            let summary = self.parse_summary_from_row(row)?;
            summaries.push(summary);
        }
        
        Ok(summaries)
    }
    
    /// Get summaries for a specific app within a timeframe
    async fn get_summaries_by_app(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        app_name: &str,
    ) -> Result<Vec<ActivitySummary>, Box<dyn Error + Send + Sync>> {
        // First, get all summaries in the timeframe
        let all_summaries = self.get_summaries_in_timeframe(start, end).await?;
        
        // Then, filter them by checking if their description or events mention the app
        let app_name_lower = app_name.to_lowercase();
        let filtered_summaries = all_summaries.into_iter()
            .filter(|summary| {
                // Check if description mentions the app
                let desc_contains = summary.description.to_lowercase().contains(&app_name_lower);
                
                // Check if any events are from the app
                let events_contain = summary.events.iter().any(|event| 
                    event.app_context.app_name.to_lowercase().contains(&app_name_lower)
                );
                
                // Check if any tags mention the app
                let tags_contain = summary.tags.iter().any(|tag| 
                    tag.to_lowercase().contains(&app_name_lower)
                );
                
                desc_contains || events_contain || tags_contain
            })
            .collect();
        
        Ok(filtered_summaries)
    }
    
    /// Search summaries by content and filter by app
    async fn search_summaries_by_app(
        &self,
        search_term: &str,
        app_name: &str,
    ) -> Result<Vec<ActivitySummary>, Box<dyn Error + Send + Sync>> {
        // First, search for summaries matching the content
        let matching_summaries = self.search_summaries(search_term).await?;
        
        // Then, filter them by app
        let app_name_lower = app_name.to_lowercase();
        let filtered_summaries = matching_summaries.into_iter()
            .filter(|summary| {
                // Check if description mentions the app
                let desc_contains = summary.description.to_lowercase().contains(&app_name_lower);
                
                // Check if any events are from the app
                let events_contain = summary.events.iter().any(|event| 
                    event.app_context.app_name.to_lowercase().contains(&app_name_lower)
                );
                
                // Check if any tags mention the app
                let tags_contain = summary.tags.iter().any(|tag| 
                    tag.to_lowercase().contains(&app_name_lower)
                );
                
                desc_contains || events_contain || tags_contain
            })
            .collect();
        
        Ok(filtered_summaries)
    }
    
    /// Parse a summary from a PostgreSQL row
    fn parse_summary_from_row(
        &self,
        row: PgRow,
    ) -> Result<ActivitySummary, Box<dyn Error + Send + Sync>> {
        let id: i32 = row.try_get("id")?;
        let start_time: DateTime<Utc> = row.try_get("start_time")?;
        let end_time: DateTime<Utc> = row.try_get("end_time")?;
        let description: String = row.try_get("description")?;
        let tags: Vec<String> = row.try_get("tags")?;
        let keystrokes: String = row.try_get("keystrokes")?;
        
        // For demonstration, create events from keystrokes
        // In a real implementation, you would fetch events from the user_events table
        let events = self.create_events_from_keystrokes(keystrokes, &start_time, &end_time)?;
        
        Ok(ActivitySummary {
            start_time,
            end_time,
            description,
            events,
            tags,
        })
    }
    
    /// Create events from keystrokes string
    /// This is a simple implementation - in production you'd query related events
    fn create_events_from_keystrokes(
        &self,
        keystrokes: String,
        start_time: &DateTime<Utc>,
        end_time: &DateTime<Utc>,
    ) -> Result<Vec<UserEvent>, Box<dyn Error + Send + Sync>> {
        // For simplicity, create one event with the keystrokes data
        let event = UserEvent {
            timestamp: *start_time,
            event: "keystroke_summary".to_string(),
            data: keystrokes,
            app_context: activity_tracker_common::AppContext {
                app_name: "Summary".to_string(),
                window_title: "Activity Summary".to_string(),
                url: None,
            },
        };
        
        Ok(vec![event])
    }

    /// Sanitize and extract key terms from the query for FTS search
    fn sanitize_query_for_search(&self, query: &str) -> String {
        // Remove question marks and other special characters
        let clean_query = query.chars()
            .filter(|c| c.is_alphanumeric() || c.is_whitespace())
            .collect::<String>();
    
        // Extract key terms (split by spaces and take words 3+ chars)
        let terms = clean_query.split_whitespace()
            .filter(|word| word.len() >= 3)
            .collect::<Vec<_>>();
    
        if terms.is_empty() {
            "user activity".to_string() // Fallback search term
        } else {
            terms.join(" OR ") // Join with OR for more permissive matching
        }
    }

    /// Parse a timeframe from a natural language query
    fn parse_timeframe(&self, query: &str) -> Timeframe {
        let now = Utc::now();
        let query = query.to_lowercase();
        
        // Today
        if query.contains("today") {
            let start = Local::now().date_naive().and_time(NaiveTime::from_hms_opt(0, 0, 0).unwrap()).and_local_timezone(Utc).unwrap();
            return Timeframe {
                start,
                end: now,
                description: "today".to_string(),
            };
        }
        
        // Yesterday
        if query.contains("yesterday") {
            let start = (Local::now() - Duration::days(1)).date_naive().and_time(NaiveTime::from_hms_opt(0, 0, 0).unwrap()).and_local_timezone(Utc).unwrap();
            let end = Local::now().date_naive().and_time(NaiveTime::from_hms_opt(0, 0, 0).unwrap()).and_local_timezone(Utc).unwrap();
            return Timeframe {
                start,
                end,
                description: "yesterday".to_string(),
            };
        }
        
        // This week
        if query.contains("this week") {
            let days_since_monday = Local::now().weekday().num_days_from_monday() as i64;
            let start = (Local::now() - Duration::days(days_since_monday)).date_naive().and_time(NaiveTime::from_hms_opt(0, 0, 0).unwrap()).and_local_timezone(Utc).unwrap();
            return Timeframe {
                start,
                end: now,
                description: "this week".to_string(),
            };
        }
        
        // Last week
        if query.contains("last week") {
            let days_since_monday = Local::now().weekday().num_days_from_monday() as i64;
            let start = (Local::now() - Duration::days(days_since_monday + 7)).date_naive().and_time(NaiveTime::from_hms_opt(0, 0, 0).unwrap()).and_local_timezone(Utc).unwrap();
            let end = (Local::now() - Duration::days(days_since_monday)).date_naive().and_time(NaiveTime::from_hms_opt(0, 0, 0).unwrap()).and_local_timezone(Utc).unwrap();
            return Timeframe {
                start,
                end,
                description: "last week".to_string(),
            };
        }
        
        // This month
        if query.contains("this month") {
            let start = Local::now().with_day(1).unwrap().with_hour(0).unwrap().with_minute(0).unwrap().with_second(0).unwrap().with_nanosecond(0).unwrap().with_timezone(&Utc);
            return Timeframe {
                start,
                end: now,
                description: "this month".to_string(),
            };
        }
        
        // Last hour
        if query.contains("last hour") || query.contains("past hour") {
            let start = now - Duration::hours(1);
            return Timeframe {
                start,
                end: now,
                description: "the last hour".to_string(),
            };
        }
        
        // Last 30 minutes
        if query.contains("30 min") || query.contains("half hour") || query.contains("half an hour") {
            let start = now - Duration::minutes(30);
            return Timeframe {
                start,
                end: now,
                description: "the last 30 minutes".to_string(),
            };
        }
        
        // Default to the last 24 hours
        let start = now - Duration::hours(24);
        Timeframe {
            start,
            end: now,
            description: "the last 24 hours".to_string(),
        }
    }
}

/// Represents a timeframe for querying data
#[derive(Debug, Clone)]
pub struct Timeframe {
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
    pub description: String,
}

/// Enum representing different types of query results
#[derive(Debug, Clone)]
pub enum QueryResult {
    Summaries {
        summaries: Vec<ActivitySummary>,
        timeframe: Timeframe,
        query: String,
        app_filter: Option<String>,
    },
    Events {
        events: Vec<UserEvent>,
        timeframe: Timeframe,
        query: String,
        app_filter: Option<String>,
    },
    Empty {
        timeframe: Timeframe,
        query: String,
        app_filter: Option<String>,
    },
}