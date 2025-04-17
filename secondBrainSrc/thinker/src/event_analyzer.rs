use activity_tracker_common::{ActivitySummary, UserEvent, llm::LlmClient};
use chrono::{DateTime, Utc};
use std::error::Error;

pub struct EventAnalyzer<T: LlmClient> {
    llm_client: T,
}

impl<T: LlmClient> EventAnalyzer<T> {
    pub fn new(llm_client: T) -> Self {
        Self { llm_client }
    }

    pub async fn analyze_events(
        &self,
        events: Vec<UserEvent>,
        start_time: DateTime<Utc>,
        end_time: DateTime<Utc>,
    ) -> Result<ActivitySummary, Box<dyn Error>> {
        // Extract key information from events for better analysis
        let mut app_count = std::collections::HashMap::new();
        let mut key_count = std::collections::HashMap::new();
        
        for event in &events {
            // Count app usage
            *app_count.entry(event.app_context.app_name.clone()).or_insert(0) += 1;
            
            // Extract and count keys from event data
            if let Ok(data) = serde_json::from_str::<serde_json::Value>(&event.data) {
                if let Some(key) = data.get("key").and_then(|k| k.as_str()) {
                    *key_count.entry(key.to_string()).or_insert(0) += 1;
                }
            }
        }
        
        // Find most used apps and keys
        let mut app_vec: Vec<_> = app_count.into_iter().collect();
        app_vec.sort_by(|a, b| b.1.cmp(&a.1));
        let top_apps = app_vec.into_iter().take(3).map(|(app, count)| format!("{} ({})", app, count)).collect::<Vec<_>>();
        
        let mut key_vec: Vec<_> = key_count.into_iter().collect();
        key_vec.sort_by(|a, b| b.1.cmp(&a.1));
        let top_keys = key_vec.into_iter().take(5).map(|(key, count)| format!("{} ({})", key, count)).collect::<Vec<_>>();
        
        // Create a rich data summary containing stats for different query types
        let stats_summary = format!(
            "Session statistics:\n\
             - Time period: {} to {}\n\
             - Total events: {}\n\
             - Top applications: {}\n\
             - Most used keys: {}\n",
            start_time.format("%H:%M"),
            end_time.format("%H:%M"),
            events.len(),
            top_apps.join(", "),
            top_keys.join(", ")
        );

        // Create a description that can be used to answer different query types
        let description = format!(
            "During this session ({} to {}), the user was active with {} events.\n\
             Most used keys: {}\n\
             Top applications: {}\n\
             Sample events: {}",
            start_time.format("%H:%M"),
            end_time.format("%H:%M"),
            events.len(),
            top_keys.join(", "),
            top_apps.join(", "),
            events.iter().take(3).map(|e| format!("{:?}", e)).collect::<Vec<_>>().join("\n")
        );

        // Extract tags from the activity data
        let tags = self.extract_tags(&description).await?;

        Ok(ActivitySummary {
            start_time,
            end_time,
            description,
            events,
            tags,
        })
    }

    async fn extract_tags(&self, description: &str) -> Result<Vec<String>, Box<dyn Error>> {
        let prompt = format!(
            "Extract 3-5 tags or topics from this activity description: \n\n{}",
            description
        );

        let tags_text = self.llm_client.generate_text(&prompt).await?;
        let tags = tags_text
            .split('\n')
            .map(|s| s.trim().to_string())
            .collect();

        Ok(tags)
    }
}
