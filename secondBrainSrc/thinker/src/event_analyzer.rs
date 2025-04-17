use activity_tracker_common::{llm::LlmClient, ActivitySummary, UserEvent};
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
        let events_text = events
            .iter()
            .map(|event| format!("{:?}", event))
            .collect::<Vec<_>>()
            .join("\n");

        let prompt = format!(
            "The following are user activity events from {} to {}. These events
            are mostly keystrokes on macos desktop. Try to string together the 
            events to figure out what might be going on. For example, if there 
            is a :wq, the user is likely trying to exit vim. A series of bash 
            commands would suggest a terminal session.\n Describe
            what the user was doing during this time period:\n\n{}\n\n",
            start_time, end_time, events_text
        );

        let description = self.llm_client.generate_text(&prompt).await?;

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
