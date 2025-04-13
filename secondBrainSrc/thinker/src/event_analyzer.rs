use activity_tracker_common::{UserEvent, ActivitySummary};
use chrono::{DateTime, Utc};
use std::error::Error;

// @todo will be using lama instead of llm_client
// also update key in thinker/main.rs
use crate::llm_client::LLMClient;

pub struct EventAnalyzer {
    llm_client: LLMClient,
}

impl EventAnalyzer {
    pub fn new(llm_client: LLMClient) -> Self {
        Self { llm_client }
    }

    pub async fn analyze_events(
        &self,
        events: Vec<UserEvent>,
        start_time: DateTime<Utc>,
        end_time: DateTime<Utc>,
    ) -> Result<ActivitySummary, Box<dyn Error>> {
        let events_text = events.iter()
            .map(|event| format!("{:?}", event))
            .collect::<Vec<_>>()
            .join("\n");

        let prompt = format!(
            "The following are user activity events from {} to {}. \n Describe what the user was doing during this time period:\n\n{}\n\n",
            start_time, end_time, events_text
        );

        let description = self.llm_client.generate_text(&prompt).await?;

        let tags = self.extract_tags(&description).await?;

        Ok(ActivitySummary {
            start_time,
            end_time,
            description,
            event_count: events.len(),
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
