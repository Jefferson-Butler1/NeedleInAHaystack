use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::Type;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[sqlx(type_name = "event_type", rename_all = "lowercase")]
pub enum EventType {
    Keystroke,
    MouseClick,
    AppSwitch,
    ScreenCapture,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserEvent {
    pub id: Option<i64>,
    pub timestamp: DateTime<Utc>,
    pub event_type: EventType,
    pub data: serde_json::Value,
    pub app_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyStrokeEvent {
    pub key: String,
    pub modifiers: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MouseClickEvent {
    pub x: i32,
    pub y: i32,
    pub button: String,
    pub target_element: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSwitchEvent {
    pub previous_app: Option<String>,
    pub current_app: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScreenCaptureEvent {
    pub window_title: String,
    pub url: Option<String>,
}
