use rdev::{listen, Event, EventType, Key};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

#[cfg(target_os = "macos")]
use std::process::Command as ProcessCommand;

#[cfg(target_os = "linux")]
use std::process::Command as ProcessCommand;

/// Enhanced event that includes the target application information
#[derive(Debug, Clone)]
pub struct EnhancedEvent {
    pub event: Event,
    pub target_app: String,
}

pub struct WindowListener {
    active_window: Arc<Mutex<String>>,
    callback: Option<Box<dyn Fn(EnhancedEvent) + Send + 'static>>,
}

impl WindowListener {
    /// Create a new WindowListener
    pub fn new() -> Self {
        let active_window = Arc::new(Mutex::new(String::new()));

        // Start a thread to periodically update the active window
        let window_tracker = Arc::clone(&active_window);
        thread::spawn(move || loop {
            let window_name = WindowListener::get_active_window();
            if let Ok(mut current) = window_tracker.lock() {
                *current = window_name;
            }
            thread::sleep(Duration::from_millis(100));
        });

        WindowListener {
            active_window,
            callback: None,
        }
    }

    /// Set the callback function that will be called for each enhanced event
    pub fn set_callback<F>(&mut self, callback: F)
    where
        F: Fn(EnhancedEvent) + Send + 'static,
    {
        self.callback = Some(Box::new(callback));
    }

    /// Start listening for events
    pub fn listen(&self) -> Result<(), rdev::ListenError> {
        let active_window = Arc::clone(&self.active_window);
        let callback = self.callback.as_ref().cloned();

        if callback.is_none() {
            return Err(rdev::ListenError::ReceiverError);
        }

        let callback = callback.unwrap();

        listen(move |event: Event| {
            let target_app = active_window.lock().unwrap().clone();
            let enhanced_event = EnhancedEvent { event, target_app };

            callback(enhanced_event);
        })
    }

    /// Get the currently active window name based on the operating system
    fn get_active_window() -> String {
        #[cfg(target_os = "macos")]
        {
            // Using AppleScript to get the active application name
            let output = ProcessCommand::new("osascript")
                .arg("-e")
                .arg("tell application \"System Events\" to get name of first application process whose frontmost is true")
                .output();

            match output {
                Ok(output) => {
                    if output.status.success() {
                        String::from_utf8_lossy(&output.stdout).trim().to_string()
                    } else {
                        "Unknown".to_string()
                    }
                }
                Err(_) => "Unknown".to_string(),
            }
        }

        #[cfg(target_os = "linux")]
        {
            // Using xdotool to get the active window name
            // First, check if xdotool is installed
            let check_xdotool = ProcessCommand::new("which").arg("xdotool").output();

            if check_xdotool.is_err() || !check_xdotool.unwrap().status.success() {
                return "Error: xdotool not installed".to_string();
            }

            // Get window ID and then window name
            let id_output = ProcessCommand::new("xdotool")
                .arg("getactivewindow")
                .output();

            match id_output {
                Ok(id_output) => {
                    if id_output.status.success() {
                        let window_id = String::from_utf8_lossy(&id_output.stdout)
                            .trim()
                            .to_string();

                        let name_output = ProcessCommand::new("xdotool")
                            .arg("getwindowname")
                            .arg(&window_id)
                            .output();

                        match name_output {
                            Ok(name_output) => {
                                if name_output.status.success() {
                                    String::from_utf8_lossy(&name_output.stdout)
                                        .trim()
                                        .to_string()
                                } else {
                                    "Unknown".to_string()
                                }
                            }
                            Err(_) => "Unknown".to_string(),
                        }
                    } else {
                        "Unknown".to_string()
                    }
                }
                Err(_) => "Unknown".to_string(),
            }
        }

        #[cfg(not(any(target_os = "macos", target_os = "linux")))]
        {
            "Unsupported OS".to_string()
        }
    }

    /// Example function to print event info with active window
    pub fn print_events() -> Result<(), rdev::ListenError> {
        let mut listener = WindowListener::new();

        listener.set_callback(|enhanced_event| {
            println!("Event: {:?}", enhanced_event.event);
            println!("Target App: {}", enhanced_event.target_app);
            println!("---");
        });

        listener.listen()
    }
}
