use chrono::{DateTime, Datelike, Duration, Local, NaiveTime, TimeZone, Timelike, Utc};

/// Timeframe utilities for parsing and managing time ranges
pub mod timeframe {
    use super::*;
    
    /// Represents a timeframe for querying data
    #[derive(Debug, Clone)]
    pub struct Timeframe {
        pub start: DateTime<Utc>,
        pub end: DateTime<Utc>,
        pub description: String,
    }
    
    /// Parse a timeframe from a natural language query
    pub fn parse_timeframe(query: &str) -> Timeframe {
        let now = Utc::now();
        let query = query.to_lowercase();
        
        // Today
        if query.contains("today") {
            let start = Local::now().date_naive().and_time(NaiveTime::from_hms_opt(0, 0, 0).unwrap()).and_local_timezone(Utc).unwrap();
            return Timeframe {
                start,
                end: now,
                description: "today".to_string(),
            };
        }
        
        // Yesterday
        if query.contains("yesterday") {
            let start = (Local::now() - Duration::days(1)).date_naive().and_time(NaiveTime::from_hms_opt(0, 0, 0).unwrap()).and_local_timezone(Utc).unwrap();
            let end = Local::now().date_naive().and_time(NaiveTime::from_hms_opt(0, 0, 0).unwrap()).and_local_timezone(Utc).unwrap();
            return Timeframe {
                start,
                end,
                description: "yesterday".to_string(),
            };
        }
        
        // This week
        if query.contains("this week") {
            let days_since_monday = Local::now().weekday().num_days_from_monday() as i64;
            let start = (Local::now() - Duration::days(days_since_monday)).date_naive().and_time(NaiveTime::from_hms_opt(0, 0, 0).unwrap()).and_local_timezone(Utc).unwrap();
            return Timeframe {
                start,
                end: now,
                description: "this week".to_string(),
            };
        }
        
        // Last week
        if query.contains("last week") {
            let days_since_monday = Local::now().weekday().num_days_from_monday() as i64;
            let start = (Local::now() - Duration::days(days_since_monday + 7)).date_naive().and_time(NaiveTime::from_hms_opt(0, 0, 0).unwrap()).and_local_timezone(Utc).unwrap();
            let end = (Local::now() - Duration::days(days_since_monday)).date_naive().and_time(NaiveTime::from_hms_opt(0, 0, 0).unwrap()).and_local_timezone(Utc).unwrap();
            return Timeframe {
                start,
                end,
                description: "last week".to_string(),
            };
        }
        
        // This month
        if query.contains("this month") {
            let start = Local::now().with_day(1).unwrap().with_hour(0).unwrap().with_minute(0).unwrap().with_second(0).unwrap().with_nanosecond(0).unwrap().with_timezone(&Utc);
            return Timeframe {
                start,
                end: now,
                description: "this month".to_string(),
            };
        }
        
        // Last hour
        if query.contains("last hour") || query.contains("past hour") {
            let start = now - Duration::hours(1);
            return Timeframe {
                start,
                end: now,
                description: "the last hour".to_string(),
            };
        }
        
        // Last 30 minutes
        if query.contains("30 min") || query.contains("half hour") || query.contains("half an hour") {
            let start = now - Duration::minutes(30);
            return Timeframe {
                start,
                end: now,
                description: "the last 30 minutes".to_string(),
            };
        }
        
        // Default to the last 24 hours
        let start = now - Duration::hours(24);
        Timeframe {
            start,
            end: now,
            description: "the last 24 hours".to_string(),
        }
    }
}

/// App detection utilities for parsing queries
pub mod app_detection {
    /// Extract application name from the query if present
    pub fn extract_app_name(query: &str) -> Option<String> {
        let query = query.to_lowercase();
        
        // Common apps to look for
        let known_apps = [
            "ghostty", "vscode", "visual studio code", "chrome", "firefox", 
            "safari", "terminal", "slack", "discord", "notion", "figma"
        ];
        
        // Check for "in {app}" or "using {app}" patterns
        for &app in &known_apps {
            let patterns = [
                format!(" in {} ", app),
                format!(" on {} ", app),
                format!(" using {} ", app),
                format!(" with {} ", app),
                format!(" at {} ", app),
            ];
            
            for pattern in &patterns {
                if query.contains(pattern) {
                    return Some(app.to_string());
                }
            }
            
            // Also check if app name is at the beginning or end of a sentence
            let start_patterns = [
                format!(" in {}.", app),
                format!(" on {}.", app),
                format!(" using {}.", app),
                format!(" with {}.", app),
                format!(" at {}.", app),
                format!(" in {}?", app),
                format!(" on {}?", app),
                format!(" using {}?", app),
                format!(" with {}?", app),
                format!(" at {}?", app),
            ];
            
            for pattern in &start_patterns {
                if query.contains(pattern) {
                    return Some(app.to_string());
                }
            }
        }
        
        None
    }
}

/// Search utilities for sanitizing and preparing search terms
pub mod search {
    /// Sanitize and extract key terms from the query for search
    pub fn sanitize_query_for_search(query: &str) -> String {
        // Remove question marks and other special characters
        let clean_query = query.chars()
            .filter(|c| c.is_alphanumeric() || c.is_whitespace())
            .collect::<String>();
    
        // Extract key terms (split by spaces and take words 3+ chars)
        let terms = clean_query.split_whitespace()
            .filter(|word| word.len() >= 3)
            .collect::<Vec<_>>();
    
        if terms.is_empty() {
            "user activity".to_string() // Fallback search term
        } else {
            terms.join(" OR ") // Join with OR for more permissive matching
        }
    }
}