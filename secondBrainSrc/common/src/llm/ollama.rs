use crate::llm::LlmClient;
use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::time::Duration;

pub struct OllamaClient {
    client: Client,
    model: String,
    base_url: String,
}

#[derive(Serialize, Debug)]
struct GenerateRequest {
    model: String,
    prompt: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    stream: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    options: Option<GenerateOptions>,
}

#[derive(Serialize, Debug, Default)]
struct GenerateOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    top_p: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    top_k: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    num_predict: Option<i32>,
}

#[derive(Deserialize, Debug)]
struct GenerateResponse {
    model: String,
    created_at: String,
    response: String,
    done: bool,
}

impl OllamaClient {
    pub async fn new(model: &str) -> Result<Self, Box<dyn Error>> {
        let client = Client::builder().timeout(Duration::from_secs(180)).build()?;

        let base_url =
            std::env::var("OLLAMA_HOST").unwrap_or_else(|_| "http://localhost:11434".to_string());
        let ollama = Self {
            client,
            model: model.to_string(),
            base_url,
        };

        ollama.check_model().await?;

        Ok(ollama)
    }

    async fn check_model(&self) -> Result<(), Box<dyn Error>> {
        let url = format!("{}/api/show", self.base_url);

        let response = self
            .client
            .post(&url)
            .json(&serde_json::json!({"name": self.model}))
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(format!(
                "Model '{}' not found in Ollama. Please check your Ollama installation.",
                self.model
            )
            .into());
        }

        Ok(())
    }
}

#[async_trait]
impl LlmClient for OllamaClient {
    async fn generate_text(&self, prompt: &str) -> Result<String, Box<dyn Error>> {
        let url = format!("{}/api/generate", self.base_url);
        // println!("{}/api/generate", self.base_url);

        let request = GenerateRequest {
            model: self.model.clone(),
            prompt: prompt.to_string(),
            stream: Some(false),
            options: Some(GenerateOptions {
                temperature: Some(0.7),
                top_p: Some(0.9),
                num_predict: Some(1024),
                ..Default::default()
            }),
        };

        let response = self
            .client
            .post(&url)
            .json(&request)
            .send()
            .await?
            .json::<GenerateResponse>()
            .await?;

        Ok(response.response.trim().to_string())
    }

    async fn extract_tags(&self, text: &str) -> Result<Vec<String>, Box<dyn Error>> {
        let prompt = format!(
            "Extract 3-5 key tags or topics from this activity description. Return each tag on a new line, without numbering or bullet points:\n\n{}",
            text
        );

        let tags_text = self.generate_text(&prompt).await?;

        let tags = tags_text
            .lines()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();

        Ok(tags)
    }
}
