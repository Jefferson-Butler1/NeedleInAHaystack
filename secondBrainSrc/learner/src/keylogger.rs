use active_win_pos_rs as active_win;
use activity_tracker_common::{AppContext, UserEvent};
use chrono::Utc;
use rdev::{listen, EventType as RdevEventType, Key};
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

            // Helper function to check if string looks like a URL
            fn is_likely_url(text: &str) -> bool {
                let text = text.trim();
                text.starts_with("http") || 
                text.starts_with("www.") || 
                text.contains(".com") ||
                text.contains(".org") ||
                text.contains(".net") ||
                text.contains(".io") ||
                text.contains(".app") ||
                text.contains(".dev")
            }

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
                                let key_str = format!("{:?}", key);

                                // Get current active window info
                                let app_context = match active_win::get_active_window() {
                                    Ok(window) => {
                                        // Debug output to see what app name actually comes through
                                        println!("Debug - Window title: {}, App name: {}", window.title, window.app_name);
                                        
                                        // Directly check for monkeytype in the title as a special case
                                        if window.title.contains("monkeytype") {
                                            // This is a monkeytype session, explicitly mark as Zen browser
                                            let modified_app_name = "Zen Browser".to_string();
                                            let browser_url = if window.title.contains("https://") {
                                                // Extract URL if present
                                                Some(window.title.to_string())
                                            } else {
                                                // Default to monkeytype website
                                                Some("https://monkeytype.com".to_string())
                                            };
                                            
                                            AppContext {
                                                app_name: modified_app_name,
                                                window_title: window.title,
                                                url: browser_url,
                                            }
                                        } else {
                                            // Normal processing for other cases
                                            let normalized_app_name = window.app_name.to_lowercase();
                                            
                                            // Detect browser type - now includes more possibilities for Zen
                                            let is_browser = normalized_app_name.contains("zen") || 
                                                            window.title.contains("mozilla") ||  // Zen is based on Mozilla
                                                            window.title.contains("firefox") ||  // Additional Firefox clues
                                                            normalized_app_name.contains("chrome") || 
                                                            normalized_app_name.contains("firefox") || 
                                                            normalized_app_name.contains("safari") || 
                                                            normalized_app_name.contains("edge") ||
                                                            normalized_app_name.contains("opera") ||
                                                            normalized_app_name.contains("brave");
                                        
                                            // Extract URL from title for browsers
                                            let browser_url = if is_browser {
                                                // Try to extract URL using several common patterns
                                                
                                                // Pattern 1: URL at beginning until separator
                                                if let Some(i) = window.title.find(" - ") {
                                                    let potential_url = window.title.split_at(i).0.trim();
                                                    if is_likely_url(potential_url) {
                                                        Some(potential_url.to_string())
                                                    } else {
                                                        None
                                                    }
                                                } 
                                                // Pattern 2: URL at end after separator
                                                else if let Some(i) = window.title.rfind(" | ") {
                                                    let potential_url = window.title.split_at(i+3).1.trim();
                                                    if is_likely_url(potential_url) {
                                                        Some(potential_url.to_string())
                                                    } else {
                                                        None
                                                    }
                                                }
                                                // Pattern 3: Title looks like a URL itself
                                                else if is_likely_url(&window.title) {
                                                    Some(window.title.clone())
                                                } 
                                                else {
                                                    None
                                                }
                                            } else {
                                                None
                                            };

                                            // Special handling for browsers to make them more identifiable
                                            let display_app_name = if is_browser && !normalized_app_name.contains("zen") {
                                                // For known browsers, make the app name clearer
                                                if window.title.contains("monkeytype") {
                                                    "Zen Browser".to_string()
                                                } else {
                                                    // Keep original name but with Browser prefix for clarity
                                                    format!("{} Browser", window.app_name)
                                                }
                                            } else {
                                                // For non-browsers or already-identified browsers, keep original name
                                                window.app_name.clone()
                                            };
                                            
                                            AppContext {
                                                app_name: display_app_name,
                                                window_title: window.title,
                                                url: browser_url,
                                            }
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