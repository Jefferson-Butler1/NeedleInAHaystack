use activity_tracker_common::{llm::LlmClient, UserEvent};
use chrono::{DateTime, Utc};
use serde_json::Value;
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
    ) -> Result<(String, Vec<String>, String), Box<dyn Error>> {
        // Extract keystrokes from events
        let keystrokes = self.extract_keystrokes(&events);

        // Generate an analysis of user activity based on keystrokes
        let description = self
            .generate_activity_description(&keystrokes, start_time, end_time)
            .await?;

        // Extract tags from the description
        let tags = self.extract_tags(&description).await?;

        Ok((description, tags, keystrokes))
    }

    fn extract_keystrokes(&self, events: &[UserEvent]) -> String {
        let mut keystrokes = String::new();

        for event in events {
            if event.event == "Keystroke" {
                // Try to parse the data as JSON
                if let Ok(value) = serde_json::from_str::<Value>(&event.data) {
                    if let Some(key) = value.get("key").and_then(|k| k.as_str()) {
                        keystrokes.push_str(key);
                    }
                } else {
                    // If not JSON, just use the data directly
                    keystrokes.push_str(&event.data);
                }
            }
        }

        keystrokes
    }

    async fn generate_activity_description(
        &self,
        keystrokes: &str,
        start_time: DateTime<Utc>,
        end_time: DateTime<Utc>,
    ) -> Result<String, Box<dyn Error>> {
        let prompt = format!(
            r#"Analyze these keystrokes from a user's activity during the period from {start} to {end}.

Keystrokes: {keystrokes}

Based on these keystrokes, what activity was the user most likely engaged in?
Look for patterns that might indicate:
1. Programming (identify language if possible)
2. Writing text/email/messages
3. Terminal commands (identify specific operations)
4. Web browsing
5. Data entry
6. Gaming

Provide a concise, factual summary of the likely activity. Focus on concrete observations from the keystroke patterns.
"#,
            start = start_time,
            end = end_time,
            keystrokes = keystrokes
        );

        self.llm_client.generate_text(&prompt).await
    }

    async fn extract_tags(&self, description: &str) -> Result<Vec<String>, Box<dyn Error>> {
        self.llm_client.extract_tags(description).await
    }
}

