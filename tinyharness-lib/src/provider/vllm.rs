use crate::provider::{ChatMessageResponse, Message, Provider, ToolDefinition};

use super::openai_compat::OpenAiCompatInner;

pub struct VllmProvider {
    inner: OpenAiCompatInner,
}

impl VllmProvider {
    pub fn new(base_url: String) -> Self {
        VllmProvider {
            inner: OpenAiCompatInner::new(base_url),
        }
    }
}

#[async_trait::async_trait]
impl Provider for VllmProvider {
    async fn health_check(&self) -> Result<(), String> {
        self.inner.health_check().await
    }

    async fn list_models(&self) -> Vec<String> {
        self.inner.fetch_model_list().await
    }

    fn select_model(&mut self, name: String) {
        self.inner.select_model(name);
    }

    fn current_model(&self) -> Option<String> {
        self.inner.current_model()
    }

    async fn chat(
        &mut self,
        messages: Vec<Message>,
        tools: Vec<ToolDefinition>,
    ) -> tokio::sync::mpsc::Receiver<ChatMessageResponse> {
        self.inner.chat(messages, tools)
    }
}
