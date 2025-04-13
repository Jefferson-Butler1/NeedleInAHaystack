use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActivitySummary {
    pub id: Option<i64>,
    pub start_time: DateTime<Utc>,
    pub end_time: DateTime<Utc>,
    pub description: String,
    pub apps_used: Vec<String>,
    pub keywords: Vec<String>,
}
