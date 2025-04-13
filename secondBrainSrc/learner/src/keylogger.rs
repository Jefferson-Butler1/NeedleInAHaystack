use activity_tracker_common::{AppContext, EventData, EventType, UserEvent};
use chrono::Utc; //@todo AVOID using Utc shit...
use std::sync::Mutex;

pub struct Keylogger {
    //@todo Could use plattform-specific library for the key capture
    // e.g., inputbot for cross-platform, windows-rs for windows etc...
}

impl Keylogger {
    pub fn new() -> Self {
        //@todo double check this later...
        Keylogger {}
    }

    pub fn poll(&self) -> Option<UserEvent> {
        //@todo implement key capture logic here
        None
    }
}
