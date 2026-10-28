//! Groq over HTTP, and the API key in Windows Credential Manager.
use std::time::Duration;

use blablaflow_core::groq::{CHAT_URL, ChatRequest, ChatResponse, MODELS_URL, Message, ModelList};

const SERVICE: &str = "blablaflow";
const USER: &str = "groq";
const TIMEOUT: Duration = Duration::from_secs(15);
const MAX_RETRY_WAIT: Duration = Duration::from_secs(2);
/// Tried in order when the user hasn't picked a model; the first one Groq still serves wins.
/// shortcut: fixed preference list; revisit when Groq's lineup changes.
const PREFERRED_MODELS: &[&str] = &["llama-3.3-70b-versatile", "openai/gpt-oss-120b", "openai/gpt-oss-20b", "llama-3.1-8b-instant"];

fn entry() -> Option<keyring::Entry> {
    keyring::Entry::new(SERVICE, USER).ok()
}

pub fn get_key() -> Option<String> {
    entry()?.get_password().ok().filter(|k| !k.trim().is_empty())
}

pub fn set_key(key: Option<&str>) -> Result<(), String> {
    let e = entry().ok_or("Credential Manager unavailable")?;
    match key.map(str::trim).filter(|k| !k.is_empty()) {
        Some(k) => e.set_password(k).map_err(|e| e.to_string()),
        None => match e.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(err) => Err(err.to_string()),
        },
    }
}

fn client() -> reqwest::Client {
    reqwest::Client::builder().timeout(TIMEOUT).build().expect("http client")
}

fn describe(e: reqwest::Error) -> String {
    if e.is_timeout() {
        "Groq timed out".into()
    } else if e.is_connect() {
        "offline".into()
    } else {
        e.to_string()
    }
}

fn status_error(status: reqwest::StatusCode) -> String {
    match status.as_u16() {
        401 | 403 => "invalid Groq API key".into(),
        429 => "Groq rate limit reached".into(),
        s => format!("Groq error {s}"),
    }
}

pub async fn chat(key: &str, model: &str, messages: Vec<Message>) -> Result<String, String> {
    let req = ChatRequest::new(model, messages);
    for attempt in 0..2 {
        let resp = client().post(CHAT_URL).bearer_auth(key).json(&req).send().await.map_err(describe)?;
        let status = resp.status();
        if status.is_success() {
            let body: ChatResponse = resp.json().await.map_err(describe)?;
            return body.text().map(str::to_string).ok_or_else(|| "empty answer from Groq".into());
        }
        let retryable = status.as_u16() == 429 || status.is_server_error();
        if attempt == 0 && retryable {
            let wait = resp
                .headers()
                .get("retry-after")
                .and_then(|v| v.to_str().ok()?.parse::<f64>().ok())
                .map_or(MAX_RETRY_WAIT, |s| Duration::from_secs_f64(s).min(MAX_RETRY_WAIT));
            tokio::time::sleep(wait).await;
            continue;
        }
        return Err(status_error(status));
    }
    unreachable!()
}

pub async fn list_models(key: &str) -> Result<Vec<String>, String> {
    let resp = client().get(MODELS_URL).bearer_auth(key).send().await.map_err(describe)?;
    if !resp.status().is_success() {
        return Err(status_error(resp.status()));
    }
    let list: ModelList = resp.json().await.map_err(describe)?;
    // Speech, TTS and guard models can't do cleanup.
    Ok(list.ids().into_iter().filter(|id| !["whisper", "tts", "guard", "playai", "orpheus"].iter().any(|x| id.contains(x))).collect())
}

pub fn pick_default_model(available: &[String]) -> Option<String> {
    PREFERRED_MODELS
        .iter()
        .find(|m| available.iter().any(|a| a == *m))
        .map(|m| m.to_string())
        .or_else(|| available.first().cloned())
}
