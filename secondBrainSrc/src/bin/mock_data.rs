use anyhow::Result;
use chrono::{DateTime, Duration, TimeZone, Utc};
use serde_json::json;
use sqlx::PgPool;
use std::env;
use tracing::info;
use tracing_subscriber::FmtSubscriber;

use second_brain::db::{init_db, timescale, general};
use second_brain::models::event::{EventType, UserEvent};
use second_brain::models::summary::ActivitySummary;

#[tokio::main]
async fn main() -> Result<()> {
    // Set up environment and logging
    dotenv::dotenv().ok();
    
    let subscriber = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;
    
    info!("Starting mock data generation");
    
    // Connect to database
    let pool = init_db().await?;
    
    // Generate mock data
    generate_mock_data(&pool).await?;
    
    info!("Mock data generation complete");
    
    Ok(())
}

async fn generate_mock_data(pool: &PgPool) -> Result<()> {
    // Generate events for the past 7 days
    let now = Utc::now();
    let week_ago = now - Duration::days(7);
    
    // Generate events with timestamps from a week ago to now
    generate_mock_events(pool, week_ago, now).await?;
    
    // Generate activity summaries
    generate_mock_summaries(pool, week_ago, now).await?;
    
    Ok(())
}

async fn generate_mock_events(pool: &PgPool, start: DateTime<Utc>, end: DateTime<Utc>) -> Result<()> {
    info!("Generating mock events");
    
    let apps = vec![
        "Firefox", "VSCode", "Terminal", "Slack", "Mail", "Notes", "Calendar"
    ];
    
    let mut current_time = start;
    while current_time < end {
        let app_index = (current_time.timestamp() % apps.len() as i64) as usize;
        let app_name = apps[app_index];
        
        // Generate keystroke events
        for _ in 0..5 {
            let event = UserEvent {
                id: None,
                timestamp: current_time,
                event_type: EventType::Keystroke,
                data: json!({
                    "key": "a",
                    "modifiers": []
                }),
                app_name: app_name.to_string(),
            };
            
            timescale::insert_event(pool, &event).await?;
            current_time = current_time + Duration::seconds(1);
        }
        
        // Generate mouse click events
        for _ in 0..3 {
            let event = UserEvent {
                id: None,
                timestamp: current_time,
                event_type: EventType::MouseClick,
                data: json!({
                    "x": 100,
                    "y": 100,
                    "button": "left",
                    "target_element": null
                }),
                app_name: app_name.to_string(),
            };
            
            timescale::insert_event(pool, &event).await?;
            current_time = current_time + Duration::seconds(1);
        }
        
        // Generate app switch event
        let next_app_index = ((app_index + 1) % apps.len()) as usize;
        let next_app = apps[next_app_index];
        
        let event = UserEvent {
            id: None,
            timestamp: current_time,
            event_type: EventType::AppSwitch,
            data: json!({
                "previous_app": app_name,
                "current_app": next_app
            }),
            app_name: next_app.to_string(),
        };
        
        timescale::insert_event(pool, &event).await?;
        
        // Jump ahead in time
        current_time = current_time + Duration::minutes(30);
    }
    
    info!("Generated mock events");
    Ok(())
}

async fn generate_mock_summaries(pool: &PgPool, start: DateTime<Utc>, end: DateTime<Utc>) -> Result<()> {
    info!("Generating mock activity summaries");
    
    // Generate summaries for each day
    let mut current_day = start.date_naive();
    let end_day = end.date_naive();
    
    while current_day <= end_day {
        let day_start = Utc.from_utc_datetime(&current_day.and_hms_opt(9, 0, 0).unwrap());
        let day_end = Utc.from_utc_datetime(&current_day.and_hms_opt(17, 0, 0).unwrap());
        
        // Morning work
        let summary1 = ActivitySummary {
            id: None,
            start_time: day_start,
            end_time: day_start + Duration::hours(3),
            description: format!("Worked on coding tasks in VSCode on {}", current_day),
            apps_used: vec!["VSCode".to_string(), "Terminal".to_string(), "Firefox".to_string()],
            keywords: vec!["coding".to_string(), "rust".to_string(), "development".to_string()],
        };
        
        general::insert_summary(pool, &summary1).await?;
        
        // Afternoon work
        let summary2 = ActivitySummary {
            id: None,
            start_time: day_start + Duration::hours(4),
            end_time: day_end,
            description: format!("Participated in meetings and responded to emails on {}", current_day),
            apps_used: vec!["Slack".to_string(), "Mail".to_string(), "Calendar".to_string()],
            keywords: vec!["meetings".to_string(), "communication".to_string(), "planning".to_string()],
        };
        
        general::insert_summary(pool, &summary2).await?;
        
        current_day = current_day.succ_opt().unwrap();
    }
    
    info!("Generated mock activity summaries");
    Ok(())
}