use async_trait::async_trait;
use serde::Serialize;

#[derive(Clone, Serialize)]
pub struct LLMMessage {
    pub role: String,
    pub content: String,
}

impl LLMMessage {
    pub fn system(content: &str) -> Self {
        Self {
            role: "system".to_owned(),
            content: content.to_owned(),
        }
    }

    pub fn user(content: &str) -> Self {
        Self {
            role: "user".to_owned(),
            content: content.to_owned(),
        }
    }
}

pub type LLMHistory = Vec<LLMMessage>;

#[async_trait]
pub trait IntoLLMHistory {
    /// convert &self into an instance of `LLMHistory`
    async fn into_llm_history(&self) -> LLMHistory;
}

#[derive(Clone)]
pub struct LLMQueryRequest {
    pub query: String,
    pub history: LLMHistory,
}

#[derive(Debug)]
pub struct LLMQueryResponse {
    pub text: String,
}

#[async_trait]
pub trait LLM
where
    Self: Sync + Send,
{
    fn model(&self) -> String;
    fn role(&self) -> String;
    async fn execute_query(&mut self, q: LLMQueryRequest) -> anyhow::Result<LLMQueryResponse>;
}
