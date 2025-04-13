use crate::models::UserEvent;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::{Pool, Postgres};
use std::error::Error;

#[async_trait]
pub trait EventStore {
    async fn store_event(&self, event: UserEvent) -> Result<(), Box<dyn Error>>;
    async fn get_events_in_timeframe(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    )-> Result<Vec<UserEvent>, Box<dyn Error>>;
}

pub struct TimescaleClient {
    pool: Pool<Postgres>,
}

impl TimescaleClient {
    pub async fn new(connection_string: &str) -> Result<Self, Box<dyn Error>>{
        let pool = sqlx::postgres::PGPoolOptions::new()
            .max_connections(5)
            .connect(connection_string)
            .await?;
    }
}

#[async_trait]
impl EventStore for TimescaleClient {
    async fn store_event(&self, event: UserEvent) -> Result<(), Box<dyn Error>> {
        //@todo implement the actual sql to store this
        println!("Storing event(IMPLEMENT THIS): {:?}", event);
        Ok(())
    }
    async fn get_events_in_timeframe(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    )-> Result<Vec<UserEvent>, Box<dyn Error>> {
        //@todo implement the actual sql to store this
        // note this will just be a get * where inside of timeframe
        println!("Getting events in timeframe(IMPLEMENT THIS): {:?} - {:?}", start, end);
        Ok(vec![])
    }
}

