use async_trait::async_trait;
use std::error::Error;

#[async_trait]
pub trait LlmClient {
    async fn generate_text(&self, prompt:&str) -> Result<String, Box<dyn Error>>;
    async fn extract_tags(&self, text:&str) -> Result<Vec<String>, Box<dyn Error>>;

}

mod ollama;
pub use ollama::OllamaClient

pub async fn create_default_client() -> Result<impl LllmClient, Box<dyn Error>> {
    ollama::OllamaClient::new("llama3.2:3b").await
}
