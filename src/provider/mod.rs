// provider/mod.rs - transcription provider abstraction.
//
// Groq's Whisper endpoint (https://api.groq.com/openai/v1/audio/transcriptions)
// is schema-identical to OpenAI's own /audio/transcriptions endpoint, so a
// single OpenAiCompatibleProvider implementation covers Groq, OpenAI, and
// any self-hosted OpenAI-compatible endpoint - only the base URL, API key
// env var, and default model differ.
//
// This also fixes the original transcribe()'s failure mode: it had no curl
// exit-code or HTTP-status check, so any network failure or bad API key
// resulted in the literal string "null" being typed at the user's cursor
// (jq -r '.text' on an error body with no .text field). Every failure path
// here is a typed ProviderError instead, so the caller can show a short
// placeholder at the cursor and log full detail, and never types raw
// provider output on failure.

mod openai_compatible;

use std::path::Path;

pub use openai_compatible::OpenAiCompatibleProvider;

use crate::config::{Config, Provider};

pub struct TranscriptionRequest<'a> {
    pub audio_path: &'a Path,
    pub model: &'a str,
    pub prompt: &'a str,
    pub language: Option<&'a str>,
}

#[derive(Debug)]
pub enum ProviderError {
    Network(reqwest::Error),
    Http { status: u16, body: String },
    MissingApiKey,
    UnexpectedResponseShape(String),
}

impl std::fmt::Display for ProviderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProviderError::Network(e) => write!(f, "network error: {e}"),
            ProviderError::Http { status, body } => write!(f, "HTTP {status}: {body}"),
            ProviderError::MissingApiKey => write!(f, "missing API key"),
            ProviderError::UnexpectedResponseShape(body) => {
                write!(f, "unexpected response shape: {body}")
            }
        }
    }
}

impl std::error::Error for ProviderError {}

pub trait TranscriptionProvider {
    fn transcribe(&self, req: &TranscriptionRequest) -> Result<String, ProviderError>;
}

const GROQ_BASE_URL: &str = "https://api.groq.com/openai/v1";
const OPENAI_BASE_URL: &str = "https://api.openai.com/v1";

/// Builds the configured provider plus the env var its API key comes from
/// (surfaced in MissingApiKey guidance rather than baked silently in).
pub fn resolve_provider(config: &Config) -> (Box<dyn TranscriptionProvider>, &'static str) {
    match config.provider {
        Provider::Groq => {
            let api_key = std::env::var("GROQ_API_KEY").unwrap_or_default();
            (
                Box::new(OpenAiCompatibleProvider {
                    base_url: GROQ_BASE_URL.to_string(),
                    api_key,
                }),
                "GROQ_API_KEY",
            )
        }
        Provider::OpenAi => {
            let api_key = std::env::var("OPENAI_API_KEY").unwrap_or_default();
            (
                Box::new(OpenAiCompatibleProvider {
                    base_url: OPENAI_BASE_URL.to_string(),
                    api_key,
                }),
                "OPENAI_API_KEY",
            )
        }
        Provider::Custom => {
            let api_key = std::env::var("RHISPER_API_KEY").unwrap_or_default();
            (
                Box::new(OpenAiCompatibleProvider {
                    base_url: config.api_base_url.clone(),
                    api_key,
                }),
                "RHISPER_API_KEY",
            )
        }
    }
}

/// Picks the model to send when the user hasn't set one explicitly via the
/// `model` config key. Preserves Groq's original duration-based large/turbo
/// split; other providers get a single sensible default.
pub fn default_model(
    provider: Provider,
    duration_secs: f64,
    long_recording_threshold: f64,
) -> &'static str {
    match provider {
        Provider::Groq => {
            if duration_secs > long_recording_threshold {
                "whisper-large-v3"
            } else {
                "whisper-large-v3-turbo"
            }
        }
        Provider::OpenAi => "whisper-1",
        Provider::Custom => "",
    }
}
