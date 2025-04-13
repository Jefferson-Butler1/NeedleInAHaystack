use anyhow::Result;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tracing::{info, error};

#[derive(Debug, Clone)]
pub struct LlmClient {
    client: Client,
    endpoint: String,
    model: String,
}

#[derive(Debug, Serialize)]
struct OllamaRequest {
    model: String,
    prompt: String,
    stream: bool,
}

#[derive(Debug, Deserialize)]
struct OllamaResponse {
    response: String,
}

impl LlmClient {
    pub fn new() -> Result<Self> {
        // By default, use Ollama's local API endpoint for llama3.2 3b
        let endpoint = "http://localhost:11434/api/generate".to_string();
        let model = "llama3.2:3b".to_string();
        
        Ok(LlmClient {
            client: Client::new(),
            endpoint,
            model,
        })
    }
    
    pub fn with_model(model: &str) -> Result<Self> {
        let endpoint = "http://localhost:11434/api/generate".to_string();
        
        Ok(LlmClient {
            client: Client::new(),
            endpoint,
            model: model.to_string(),
        })
    }
    
    pub async fn generate(&self, prompt: &str) -> Result<String> {
        info!("Generating text with Ollama LLM using model: {}", self.model);
        
        let request = OllamaRequest {
            model: self.model.clone(),
            prompt: prompt.to_string(),
            stream: false,
        };
        
        let response = self.client.post(&self.endpoint)
            .json(&request)
            .send()
            .await?;
            
        if !response.status().is_success() {
            let error_text = response.text().await?;
            error!("Ollama API error: {}", error_text);
            return Err(anyhow::anyhow!("Ollama API error: {}", error_text));
        }
        
        let ollama_response: OllamaResponse = response.json().await?;
        
        Ok(ollama_response.response.trim().to_string())
    }
}
