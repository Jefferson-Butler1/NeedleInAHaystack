use active_win_pos_rs as active_win;
use activity_tracker_common::{AppContext, UserEvent};
use chrono::Utc;
use rdev::{listen, EventType as RdevEventType, Key};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::thread;
use tokio::sync::mpsc;

// Maximum number of events to hold in the buffer
const MAX_BUFFER_SIZE: usize = 100;

// We store key data directly in JSON format now, so we don't need this struct anymore
// But keeping the structure commented for future reference if needed
// #[derive(Debug, Clone)]
// struct KeyEvent {
//     key: String,
//     modifiers: Vec<String>,
//     timestamp: chrono::DateTime<Utc>,
// }

pub struct Keylogger {
    event_buffer: Arc<Mutex<VecDeque<UserEvent>>>,
    _rx: Option<mpsc::Receiver<()>>, // Kept to maintain the channel open
}

impl Keylogger {
    pub fn new() -> Self {
        let event_buffer = Arc::new(Mutex::new(VecDeque::with_capacity(MAX_BUFFER_SIZE)));
        let buffer_clone = event_buffer.clone();

        // Setup MPSC channel to allow for clean shutdown if needed
        let (_tx, rx) = mpsc::channel(1);

        // Spawn a thread to listen for keyboard events
        thread::spawn(move || {
            // Track modifier key states
            let mut shift_pressed = false;
            let mut ctrl_pressed = false;
            let mut alt_pressed = false;
            let mut meta_pressed = false;

            // Callback that processes each keyboard event
            if let Err(error) = listen(move |event| {
                match event.event_type {
                    RdevEventType::KeyPress(key) => {
                        // Update modifier state
                        match key {
                            Key::ShiftLeft | Key::ShiftRight => shift_pressed = true,
                            Key::ControlLeft | Key::ControlRight => ctrl_pressed = true,
                            Key::Alt | Key::AltGr => alt_pressed = true,
                            Key::MetaLeft | Key::MetaRight => meta_pressed = true,
                            _ => {
                                // For normal keys, record the event
                                // Convert key to a string representation
                                let key_str = format!("{:?}", key);
                                
                                // Get current active window info
                                let app_context = match active_win::get_active_window() {
                                        Ok(window) => {
                                            // Extract URL from title for common browsers (simple heuristic)
                                            let browser_url = if window
                                                .app_name
                                                .to_lowercase()
                                                .contains("chrome")
                                                || window
                                                    .app_name
                                                    .to_lowercase()
                                                    .contains("firefox")
                                                || window.app_name.to_lowercase().contains("safari")
                                                || window.app_name.to_lowercase().contains("edge")
                                            {
                                                // Try to extract URL from title (very basic)
                                                if let Some(i) = window.title.find(" - ") {
                                                    let potential_url =
                                                        window.title.split_at(i).0.trim();
                                                    if potential_url.starts_with("http")
                                                        || potential_url.contains("www.")
                                                        || potential_url.contains(".com")
                                                    {
                                                        Some(potential_url.to_string())
                                                    } else {
                                                        None
                                                    }
                                                } else {
                                                    None
                                                }
                                            } else {
                                                None
                                            };

                                            AppContext {
                                                app_name: window.app_name,
                                                window_title: window.title,
                                                url: browser_url,
                                            }
                                        }
                                        Err(_) => AppContext {
                                            app_name: "unknown".to_string(),
                                            window_title: "unknown".to_string(),
                                            url: None,
                                        },
                                    };

                                    // Build modifiers list
                                    let mut modifiers = Vec::new();
                                    if shift_pressed {
                                        modifiers.push("Shift".to_string());
                                    }
                                    if ctrl_pressed {
                                        modifiers.push("Ctrl".to_string());
                                    }
                                    if alt_pressed {
                                        modifiers.push("Alt".to_string());
                                    }
                                    if meta_pressed {
                                        modifiers.push("Meta".to_string());
                                    }

                                    // Create key data as JSON
                                    let key_data = serde_json::json!({
                                        "key": key_str,
                                        "modifiers": modifiers
                                    })
                                    .to_string();

                                    // Create the user event
                                    let event = UserEvent {
                                        timestamp: Utc::now(),
                                        event: "keystroke".to_string(),
                                        data: key_data,
                                        app_context,
                                    };

                                    // Add to buffer
                                    let mut buffer = buffer_clone.lock().unwrap();
                                    buffer.push_back(event);

                                    // If buffer is full, remove oldest event
                                    if buffer.len() > MAX_BUFFER_SIZE {
                                        buffer.pop_front();
                                    }
                            }
                        }
                    }
                    RdevEventType::KeyRelease(key) => {
                        // Update modifier state
                        match key {
                            Key::ShiftLeft | Key::ShiftRight => shift_pressed = false,
                            Key::ControlLeft | Key::ControlRight => ctrl_pressed = false,
                            Key::Alt | Key::AltGr => alt_pressed = false,
                            Key::MetaLeft | Key::MetaRight => meta_pressed = false,
                            _ => {}
                        }
                    }
                    _ => {} // Ignore other event types
                }
            }) {
                eprintln!("Error: {:?}", error);
            }
        });

        Keylogger {
            event_buffer,
            _rx: Some(rx),
        }
    }

    pub fn poll(&self) -> Option<UserEvent> {
        let mut buffer = self.event_buffer.lock().unwrap();
        buffer.pop_front()
    }
}
