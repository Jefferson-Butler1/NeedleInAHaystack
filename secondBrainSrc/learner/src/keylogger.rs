use activity_tracker_common::{listener, AppContext, UserEvent};
use chrono::Utc;
use rdev::{EventType as RdevEventType, Key};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::thread;
use tokio::sync::mpsc;

const MAX_BUFFER_SIZE: usize = 1000;

pub struct Keylogger {
    event_buffer: Arc<Mutex<VecDeque<UserEvent>>>,
    _rx: Option<mpsc::Receiver<()>>,
}

impl Keylogger {
    pub fn new() -> Self {
        let event_buffer = Arc::new(Mutex::new(VecDeque::with_capacity(MAX_BUFFER_SIZE)));
        let buffer_clone = event_buffer.clone();

        // Setup MPSC channel to allow for clean shutdown if needed
        let (_tx, rx) = mpsc::channel(1);

        thread::spawn(move || {
            // Track modifier key states
            let mut shift_pressed = false;
            let mut ctrl_pressed = false;
            let mut alt_pressed = false;
            let mut meta_pressed = false;

            // Process keyboard events
            let window_listener = listener::WindowListener::new();
            if let Err(error) = window_listener.listen_with_callback(move |event| {
                println!("Event: {:?}", event);
                match event.event.event_type {
                    RdevEventType::KeyPress(key) => {
                        // Update modifier key state
                        match key {
                            Key::ShiftLeft | Key::ShiftRight => {
                                shift_pressed = true;
                            }
                            Key::ControlLeft | Key::ControlRight => {
                                ctrl_pressed = true;
                            }
                            Key::Alt | Key::AltGr => {
                                alt_pressed = true;
                            }
                            Key::MetaLeft | Key::MetaRight => {
                                meta_pressed = true;
                            }
                            _ => {
                                // For non-modifier keys, record the event
                                let key_str = format!("{:?}", key);

                                // Get window info from the enhanced event
                                println!("Keystroke: {} in app: {}", key_str, event.target_app);

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

                                // Get window title if available or generate one from app name
                                let window_title = event.window_title.clone().unwrap_or_else(|| 
                                    extract_window_title_from_target(&event.target_app)
                                );
                                let url = extract_url_from_title(&window_title, &event.target_app);

                                // Create app context from the target_app
                                let app_context = AppContext {
                                    app_name: event.target_app.clone(),
                                    window_title,
                                    url,
                                };

                                // Create the user event with the active window info
                                let user_event = UserEvent {
                                    timestamp: Utc::now(),
                                    event: "keystroke".to_string(),
                                    data: key_data,
                                    app_context,
                                };

                                // Add to buffer
                                let mut buffer = buffer_clone.lock().unwrap();
                                buffer.push_back(user_event);

                                // If buffer is full, remove oldest event
                                if buffer.len() > MAX_BUFFER_SIZE {
                                    buffer.pop_front();
                                }
                            }
                        }
                    }
                    RdevEventType::KeyRelease(key) => {
                        // Update modifier key state
                        match key {
                            Key::ShiftLeft | Key::ShiftRight => shift_pressed = false,
                            Key::ControlLeft | Key::ControlRight => ctrl_pressed = false,
                            Key::Alt | Key::AltGr => alt_pressed = false,
                            Key::MetaLeft | Key::MetaRight => meta_pressed = false,
                            _ => {}
                        }
                    }
                    _ => {} // Ignore other event types like mouse movements
                }
            }) {
                eprintln!("Error in keylogger: {:?}", error);
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

/// Extract window title from target app information
/// Since the WindowListener only gives us the app name, we'll do our best to construct a meaningful title
fn extract_window_title_from_target(target_app: &str) -> String {
    // For now, we just use the app name as the title
    // In a real implementation, we'd need to enhance the WindowListener to include window title
    target_app.to_string()
}

/// Extracts a URL from a window title if one exists
fn extract_url_from_title(title: &str, app_name: &str) -> Option<String> {
    let lowercase_app = app_name.to_lowercase();
    let is_browser = lowercase_app.contains("zen")
        || lowercase_app.contains("chrome")
        || lowercase_app.contains("firefox")
        || lowercase_app.contains("safari");

    if !is_browser {
        return None;
    }

    // Common URL patterns in browser titles

    // Pattern 1: Title starts with URL
    if title.starts_with("http://") || title.starts_with("https://") {
        if let Some(i) = title.find(" ") {
            return Some(title[0..i].to_string());
        }
    }

    // Pattern 2: URL followed by separator
    if let Some(i) = title.find(" - ") {
        let potential_url = &title[0..i];
        if potential_url.contains(".com")
            || potential_url.contains(".org")
            || potential_url.contains(".net")
        {
            return Some(potential_url.to_string());
        }
    }

    None
}

