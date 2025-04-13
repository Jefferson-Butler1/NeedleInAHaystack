use anyhow::Result;
use chrono::Utc;
use rdev::{Event, EventType as RdevEventType, listen};
use serde_json::json;
use sqlx::{Pool, Postgres};
use std::sync::{Arc, Mutex};
use std::thread;
use tokio::sync::mpsc;
use tracing::{info, error};

use crate::db::timescale;
use crate::models::event::{EventType, UserEvent, KeyStrokeEvent, MouseClickEvent, AppSwitchEvent};
use crate::utils::app_info::get_active_app;

pub struct Learner {
    db_pool: Pool<Postgres>,
    event_sender: mpsc::Sender<UserEvent>,
    active_app: Arc<Mutex<String>>,
}

impl Learner {
    pub fn new(db_pool: Pool<Postgres>, event_sender: mpsc::Sender<UserEvent>) -> Self {
        Learner {
            db_pool,
            event_sender,
            active_app: Arc::new(Mutex::new(String::new())),
        }
    }

    pub async fn start(&self) -> Result<()> {
        info!("Starting learner thread");
        
        // Start the active app checking thread
        let app_checker = self.spawn_app_checker();
        
        // Start event listener in a separate thread
        let event_sender = self.event_sender.clone();
        let active_app = self.active_app.clone();
        
        thread::spawn(move || {
            if let Err(err) = listen(move |event| {
                Self::handle_event(&event, &event_sender, &active_app);
            }) {
                error!("Error in event listener: {:?}", err);
            }
        });
        
        app_checker.await?;
        Ok(())
    }
    
    fn spawn_app_checker(&self) -> tokio::task::JoinHandle<Result<()>> {
        let event_sender = self.event_sender.clone();
        let active_app = self.active_app.clone();
        
        tokio::spawn(async move {
            let mut last_app = String::new();
            
            loop {
                tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                
                let current_app = get_active_app()?;
                
                if current_app != last_app {
                    // Update the active app
                    {
                        let mut app = active_app.lock().unwrap();
                        *app = current_app.clone();
                    }
                    
                    // Create and send an app switch event
                    let app_switch = AppSwitchEvent {
                        previous_app: Some(last_app.clone()),
                        current_app: current_app.clone(),
                    };
                    
                    let event = UserEvent {
                        id: None,
                        timestamp: Utc::now(),
                        event_type: EventType::AppSwitch,
                        data: json!(app_switch),
                        app_name: current_app.clone(),
                    };
                    
                    if let Err(e) = event_sender.send(event).await {
                        error!("Failed to send app switch event: {}", e);
                    }
                    
                    last_app = current_app;
                }
            }
        })
    }
    
    fn handle_event(event: &Event, sender: &mpsc::Sender<UserEvent>, active_app: &Arc<Mutex<String>>) {
        let app_name = active_app.lock().unwrap().clone();
        if app_name.is_empty() {
            return;
        }
        
        let event_data = match &event.event_type {
            RdevEventType::KeyPress(key) => {
                let keystroke = KeyStrokeEvent {
                    key: format!("{:?}", key),
                    modifiers: vec![], // Would need to track modifier state
                };
                
                Some((EventType::Keystroke, json!(keystroke)))
            },
            RdevEventType::ButtonPress(button) => {
                // rdev doesn't provide direct position access, so we'll use dummy values
                let mouse_click = MouseClickEvent {
                    x: 0,
                    y: 0,
                    button: format!("{:?}", button),
                    target_element: None, // Would need additional processing
                };
                
                Some((EventType::MouseClick, json!(mouse_click)))
            },
            _ => None,
        };
        
        if let Some((event_type, data)) = event_data {
            let user_event = UserEvent {
                id: None,
                timestamp: Utc::now(),
                event_type,
                data,
                app_name,
            };
            
            let sender_clone = sender.clone();
            tokio::spawn(async move {
                if let Err(e) = sender_clone.send(user_event).await {
                    error!("Failed to send event: {}", e);
                }
            });
        }
    }
    
    pub async fn process_events(&self) -> Result<()> {
        info!("Starting event processor");
        
        let mut receiver = mpsc::channel(100).1;
        
        // In a real implementation, this would be connected to the sender
        while let Some(event) = receiver.recv().await {
            match timescale::insert_event(&self.db_pool, &event).await {
                Ok(_) => info!("Successfully stored event"),
                Err(e) => error!("Failed to store event: {}", e),
            }
        }
        
        Ok(())
    }
}