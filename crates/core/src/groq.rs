//! Groq's OpenAI-compatible request/response shapes. HTTP lives in the app.
use serde::{Deserialize, Serialize};

pub const CHAT_URL: &str = "https://api.groq.com/openai/v1/chat/completions";
pub const MODELS_URL: &str = "https://api.groq.com/openai/v1/models";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Message {
    pub role: String,
    pub content: String,
}

impl Message {
    pub fn system(content: impl Into<String>) -> Self {
        Self { role: "system".into(), content: content.into() }
    }
    pub fn user(content: impl Into<String>) -> Self {
        Self { role: "user".into(), content: content.into() }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ChatRequest {
    pub model: String,
    pub messages: Vec<Message>,
    pub temperature: f32,
    pub max_tokens: u32,
    /// Reasoning models think before answering; for dictation cleanup that's pure latency.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning_effort: Option<&'static str>,
}

impl ChatRequest {
    pub fn new(model: impl Into<String>, messages: Vec<Message>) -> Self {
        let model = model.into();
        let reasoning_effort = if model.starts_with("openai/gpt-oss") {
            Some("low")
        } else if model.contains("qwen3") {
            Some("none")
        } else {
            None
        };
        Self { model, messages, temperature: 0.2, max_tokens: 4096, reasoning_effort }
    }
}

#[derive(Debug, Deserialize)]
pub struct ChatResponse {
    choices: Vec<Choice>,
}

#[derive(Debug, Deserialize)]
struct Choice {
    message: Message,
}

impl ChatResponse {
    pub fn text(&self) -> Option<&str> {
        self.choices.first().map(|c| c.message.content.as_str())
    }
}

#[derive(Debug, Deserialize)]
pub struct ModelList {
    data: Vec<ModelInfo>,
}

#[derive(Debug, Deserialize)]
struct ModelInfo {
    id: String,
}

impl ModelList {
    /// Sorted model ids.
    pub fn ids(&self) -> Vec<String> {
        let mut v: Vec<String> = self.data.iter().map(|m| m.id.clone()).collect();
        v.sort();
        v
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chat_request_serializes() {
        let r = ChatRequest::new("m", vec![Message::system("s"), Message::user("u")]);
        let v = serde_json::to_value(&r).unwrap();
        assert_eq!(v["model"], "m");
        assert_eq!(v["messages"][1]["role"], "user");
        assert_eq!(v["max_tokens"], 4096);
        assert!((v["temperature"].as_f64().unwrap() - 0.2).abs() < 1e-6);
    }

    #[test]
    fn reasoning_models_think_as_little_as_possible() {
        let v = |m: &str| serde_json::to_value(ChatRequest::new(m, vec![])).unwrap();
        assert_eq!(v("openai/gpt-oss-120b")["reasoning_effort"], "low");
        assert_eq!(v("qwen/qwen3.8-27b")["reasoning_effort"], "none");
        assert!(v("llama-3.1-8b-instant").get("reasoning_effort").is_none());
    }

    #[test]
    fn chat_response_parses_groq_json() {
        let json = r#"{"id":"x","object":"chat.completion","created":1,"model":"m",
            "choices":[{"index":0,"message":{"role":"assistant","content":"Hello."},"finish_reason":"stop"}],
            "usage":{"prompt_tokens":1,"completion_tokens":1,"total_tokens":2}}"#;
        let r: ChatResponse = serde_json::from_str(json).unwrap();
        assert_eq!(r.text(), Some("Hello."));
    }

    #[test]
    fn model_list_parses() {
        let json = r#"{"object":"list","data":[{"id":"b","object":"model","active":true},{"id":"a","object":"model"}]}"#;
        let m: ModelList = serde_json::from_str(json).unwrap();
        assert_eq!(m.ids(), vec!["a", "b"]);
    }
}
