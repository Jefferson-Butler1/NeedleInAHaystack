use anyhow::Result;
use chrono::{DateTime, Duration, Utc};
use sqlx::{Pool, Postgres};
use tokio::time;
use tracing::{info, error};

use crate::db::{timescale, general};
use crate::models::event::UserEvent;
use crate::models::summary::ActivitySummary;
use crate::utils::llm::LlmClient;

pub struct Thinker {
    db_pool: Pool<Postgres>,
    llm_client: LlmClient,
    processing_interval: Duration,
}

impl Thinker {
    pub fn new(
        db_pool: Pool<Postgres>,
        llm_client: LlmClient,
        processing_interval_minutes: i64,
    ) -> Self {
        Thinker {
            db_pool,
            llm_client,
            processing_interval: Duration::minutes(processing_interval_minutes),
        }
    }

    pub async fn start(&self) -> Result<()> {
        info!("Starting thinker thread");
        
        let mut interval = time::interval(tokio::time::Duration::from_secs(60));
        let mut last_processed_time = Utc::now() - self.processing_interval;
        
        loop {
            interval.tick().await;
            
            let current_time = Utc::now();
            if current_time - last_processed_time >= self.processing_interval {
                info!("Processing events from {:?} to {:?}", last_processed_time, current_time);
                
                if let Err(e) = self.process_time_window(last_processed_time, current_time).await {
                    error!("Error processing time window: {}", e);
                } else {
                    last_processed_time = current_time;
                }
            }
        }
    }
    
    async fn process_time_window(
        &self,
        start_time: DateTime<Utc>,
        end_time: DateTime<Utc>,
    ) -> Result<()> {
        // Fetch all events in the given time window
        let events = timescale::get_events_in_timeframe(&self.db_pool, start_time, end_time).await?;
        
        if events.is_empty() {
            info!("No events to process in the time window");
            return Ok(());
        }
        
        // Extract unique app names
        let apps_used: Vec<String> = events.iter()
            .map(|e| e.app_name.clone())
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();
        
        // Generate a description using LLM
        let description = self.generate_summary(&events).await?;
        
        // Extract keywords from the description
        let keywords = self.extract_keywords(&description).await?;
        
        // Create and store the activity summary
        let summary = ActivitySummary {
            id: None,
            start_time,
            end_time,
            description,
            apps_used,
            keywords,
        };
        
        general::insert_summary(&self.db_pool, &summary).await?;
        
        info!("Successfully created and stored activity summary");
        Ok(())
    }
    
    async fn generate_summary(&self, events: &[UserEvent]) -> Result<String> {
        // Format events for the LLM
        let events_text = events.iter()
            .map(|e| format!(
                "[{}] {} in {}: {:?}",
                e.timestamp,
                match e.event_type {
                    crate::models::event::EventType::Keystroke => "Keystroke",
                    crate::models::event::EventType::MouseClick => "MouseClick",
                    crate::models::event::EventType::AppSwitch => "AppSwitch",
                    crate::models::event::EventType::ScreenCapture => "ScreenCapture",
                },
                e.app_name,
                e.data
            ))
            .collect::<Vec<_>>()
            .join("\n");
        
        let prompt = format!(
            "Below is a series of user events from a computer. \
            Please summarize what the user was doing during this time in 1-2 sentences. \
            Focus on the tasks and goals, not the specific keystrokes or clicks:\n\n{}",
            events_text
        );
        
        self.llm_client.generate(&prompt).await
    }
    
    async fn extract_keywords(&self, description: &str) -> Result<Vec<String>> {
        let prompt = format!(
            "Extract 3-7 keywords from the following activity description. \
            Return only the keywords as a comma-separated list with no additional text:\n\n{}",
            description
        );
        
        let response = self.llm_client.generate(&prompt).await?;
        
        // Parse comma-separated keywords
        let keywords = response.split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        
        Ok(keywords)
    }
}