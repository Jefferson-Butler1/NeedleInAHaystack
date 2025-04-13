use anyhow::Result;
use chrono::{DateTime, Duration, Utc};
use fuzzy_matcher::FuzzyMatcher;
use fuzzy_matcher::skim::SkimMatcherV2;
use sqlx::{Pool, Postgres};
use tracing::info;

use crate::db::general;
use crate::models::summary::ActivitySummary;
use crate::utils::llm::LlmClient;

pub struct Recall {
    db_pool: Pool<Postgres>,
    llm_client: LlmClient,
}

impl Recall {
    pub fn new(db_pool: Pool<Postgres>, llm_client: LlmClient) -> Self {
        Recall {
            db_pool,
            llm_client,
        }
    }

    // Query by fuzzy matching against descriptions and keywords
    pub async fn fuzzy_search(&self, query: &str) -> Result<Vec<ActivitySummary>> {
        info!("Performing fuzzy search for: {}", query);
        
        // First try a direct database search
        let db_results = general::search_summaries(&self.db_pool, query).await?;
        
        if !db_results.is_empty() {
            return Ok(db_results);
        }
        
        // If no direct matches, fetch recent summaries and apply fuzzy matching
        let one_month_ago = Utc::now() - Duration::days(30);
        let all_recent = general::get_summaries_in_timeframe(
            &self.db_pool, 
            one_month_ago, 
            Utc::now()
        ).await?;
        
        // Set up fuzzy matcher
        let matcher = SkimMatcherV2::default();
        
        // Score each summary by description match
        let mut scored_results: Vec<(i64, ActivitySummary)> = all_recent.into_iter()
            .filter_map(|summary| {
                // Try to match against description
                let score = matcher.fuzzy_match(&summary.description, query);
                
                // Or against any keyword
                let keyword_score = summary.keywords.iter()
                    .filter_map(|keyword| matcher.fuzzy_match(keyword, query))
                    .max();
                
                // Use the higher score
                match (score, keyword_score) {
                    (Some(s1), Some(s2)) => Some((std::cmp::max(s1, s2), summary)),
                    (Some(s), None) => Some((s, summary)),
                    (None, Some(s)) => Some((s, summary)),
                    (None, None) => None,
                }
            })
            .collect();
        
        // Sort by score (descending)
        scored_results.sort_by(|a, b| b.0.cmp(&a.0));
        
        // Return top results (or all if less than 10)
        let top_results = scored_results.into_iter()
            .map(|(_, summary)| summary)
            .take(10)
            .collect();
        
        Ok(top_results)
    }
    
    // Natural language query that gets translated to a time range or search query
    pub async fn natural_language_query(&self, query: &str) -> Result<Vec<ActivitySummary>> {
        info!("Processing natural language query: {}", query);
        
        // Use LLM to interpret the query
        let interpret_prompt = format!(
            "Parse the following query about user activities and convert it to either:\n\
            1. A time range (start_date and end_date in ISO format)\n\
            2. A search term or topic\n\
            Return your answer in JSON format like this: {{\"type\": \"timerange\", \"start\": \"ISO_DATE\", \"end\": \"ISO_DATE\"}} \
            or {{\"type\": \"search\", \"term\": \"search term\"}}\n\n\
            Query: {}", 
            query
        );
        
        let interpretation = self.llm_client.generate(&interpret_prompt).await?;
        
        // Parse the JSON response
        let parsed: serde_json::Value = serde_json::from_str(&interpretation)?;
        
        match parsed["type"].as_str() {
            Some("timerange") => {
                // Extract time range and query database
                let start_str = parsed["start"].as_str().ok_or_else(|| 
                    anyhow::anyhow!("Missing start date in LLM response"))?;
                let end_str = parsed["end"].as_str().ok_or_else(|| 
                    anyhow::anyhow!("Missing end date in LLM response"))?;
                
                let start = DateTime::parse_from_rfc3339(start_str)?.with_timezone(&Utc);
                let end = DateTime::parse_from_rfc3339(end_str)?.with_timezone(&Utc);
                
                general::get_summaries_in_timeframe(&self.db_pool, start, end).await
            },
            Some("search") => {
                // Extract search term and perform search
                let term = parsed["term"].as_str().ok_or_else(|| 
                    anyhow::anyhow!("Missing search term in LLM response"))?;
                
                self.fuzzy_search(term).await
            },
            _ => Err(anyhow::anyhow!("Invalid response format from LLM"))
        }
    }
    
    // Summarize a collection of activity summaries
    pub async fn meta_summarize(&self, summaries: &[ActivitySummary]) -> Result<String> {
        if summaries.is_empty() {
            return Ok("No activities found for the specified criteria.".to_string());
        }
        
        let activities_text = summaries.iter()
            .map(|s| format!(
                "[{} to {}] {}", 
                s.start_time.format("%Y-%m-%d %H:%M"), 
                s.end_time.format("%Y-%m-%d %H:%M"),
                s.description
            ))
            .collect::<Vec<_>>()
            .join("\n");
        
        let prompt = format!(
            "Below are summaries of user activities over time. \
            Please create a concise summary that explains the overall pattern and main focuses.\n\n{}",
            activities_text
        );
        
        self.llm_client.generate(&prompt).await
    }
}