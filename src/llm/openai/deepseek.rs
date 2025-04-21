use crate::llm::{LLMMessage, LLMQueryRequest, LLMQueryResponse, LLM};
use anyhow::Context;
use async_trait::async_trait;
use deepseek_rs::{
    request::{Message, RequestBody, ResponseFormat, ResponseFormatType, Role, Temperature},
    DeepSeekClient,
};

pub struct Deepseek {
    history: Vec<LLMMessage>,
    client: DeepSeekClient,
}

impl Deepseek {
    pub fn new(prompt: &str) -> Self {
        let history = {
            let mut h = Vec::new();
            h.push(Self::build_prompt(&prompt));
            h
        };

        Self {
            history,
            client: DeepSeekClient::default().unwrap(),
        }
    }

    fn build_prompt(prompt: &str) -> LLMMessage {
        LLMMessage {
            role: "system".into(),
            content: prompt.into(),
        }
    }

    fn create_user_message(&self, content: &str) -> LLMMessage {
        LLMMessage {
            role: "user".into(),
            content: content.into(),
        }
    }

    async fn execute(&mut self) -> anyhow::Result<String> {
        let messages = self
            .history
            .iter()
            .map(|m| {
                let role = match m.role.as_str() {
                    "system" => Role::System,
                    "assistant" => Role::Assistant,
                    _ => Role::User,
                };

                Message::new(role, m.content.to_owned(), None)
            })
            .collect::<Vec<Message>>();

        let request = RequestBody::default()
            .with_messages(messages)
            .with_model(deepseek_rs::request::Model::DeepseekChat)
            .with_temperature(Temperature::new(0.0))
            .with_response_format(ResponseFormat::new(ResponseFormatType::Json));

        //::new_messages(vec![Message::new_user_message("Hello".to_string())]);
        let result = self
            .client
            .chat_completions(request)
            .await
            .context("failed to execute")?;

        let content = result
            .choices
            .get(0)
            .unwrap()
            .message
            .content
            .clone()
            .unwrap();
        self.history.push(LLMMessage {
            role: "assistant".to_owned(),
            content: content.clone(),
        });

        Ok(content.clone())
    }
}

#[async_trait]
impl LLM for Deepseek {
    fn role(&self) -> String {
        "system".to_owned()
    }

    fn model(&self) -> String {
        "deepseek-reasoner".to_owned()
    }

    async fn execute_query(&mut self, req: LLMQueryRequest) -> anyhow::Result<LLMQueryResponse> {
        self.history.push(self.create_user_message(&req.query));
        let text = self.execute().await?;
        Ok(LLMQueryResponse { text })
    }
}