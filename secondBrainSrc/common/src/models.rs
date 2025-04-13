use chrono::{DateTime, Utc}
use serde::{Deserialize, Serialize}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserEvent {
    pub timestam: DateTime<Utc>,
    pub event: String,
    pub data: String,
    pub app_context: AppContext,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EventType {
    Keystroke, 
    // MouseClick,
    // AppSwitch
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EventData {
    Keystroke {key: String, modifiers: Vec<String>}
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActivitySummary {
    pub start_time: DateTime<Utc>,
    pub end_time: DateTime<Utc>,
    pub description: String,
    pub events: Vec<UserEvent>,
    pub tags: Vec<String>
}
