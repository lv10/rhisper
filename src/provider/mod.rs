// provider/mod.rs - transcription provider abstraction.
//
// Groq's Whisper endpoint (https://api.groq.com/openai/v1/audio/transcriptions)
// is schema-identical to OpenAI's own /audio/transcriptions endpoint, so a
// single OpenAiCompatibleProvider implementation covers Groq, OpenAI, and
// any self-hosted OpenAI-compatible endpoint - only the base URL, API key
// env var, and default model differ. Gemini has no such endpoint, so it has
// its own GeminiProvider that calls the native generateContent API.
//
// This also fixes the original transcribe()'s failure mode: it had no curl
// exit-code or HTTP-status check, so any network failure or bad API key
// resulted in the literal string "null" being typed at the user's cursor
// (jq -r '.text' on an error body with no .text field). Every failure path
// here is a typed ProviderError instead, so the caller can show a short
// placeholder at the cursor and log full detail, and never types raw
// provider output on failure.

mod gemini;
mod openai_compatible;

use std::path::Path;

pub use gemini::GeminiProvider;
pub use openai_compatible::OpenAiCompatibleProvider;

use crate::config::{Config, Provider};
use crate::polish::PolishContext;

#[derive(Clone, Copy)]
pub struct TranscriptionRequest<'a> {
    pub audio_path: &'a Path,
    pub model: &'a str,
    pub prompt: &'a str,
    pub language: Option<&'a str>,
    /// Set when the transcript should be lightly polished. Only Gemini
    /// reads it; the Whisper-style providers ignore it.
    pub polish: Option<&'a PolishContext>,
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

/// Transcribes, polishing if the request asks for it. A failed polish must
/// not lose the dictation, so any error except a missing key is retried once
/// as a plain verbatim request. Returns the final result plus the polished
/// attempt's error (if it failed) so the caller can log it.
pub fn transcribe_with_fallback(
    provider: &dyn TranscriptionProvider,
    req: &TranscriptionRequest,
) -> (Result<String, ProviderError>, Option<ProviderError>) {
    let first = provider.transcribe(req);
    match first {
        Err(e) if req.polish.is_some() && !matches!(e, ProviderError::MissingApiKey) => {
            let verbatim = TranscriptionRequest {
                polish: None,
                ..*req
            };
            (provider.transcribe(&verbatim), Some(e))
        }
        other => (other, None),
    }
}

const GROQ_BASE_URL: &str = "https://api.groq.com/openai/v1";
const OPENAI_BASE_URL: &str = "https://api.openai.com/v1";
const GEMINI_BASE_URL: &str = "https://generativelanguage.googleapis.com/v1beta";

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
        Provider::Gemini => {
            let api_key = std::env::var("GEMINI_API_KEY").unwrap_or_default();
            (
                Box::new(GeminiProvider {
                    base_url: GEMINI_BASE_URL.to_string(),
                    api_key,
                }),
                "GEMINI_API_KEY",
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
        Provider::Gemini => "gemini-3.5-flash-lite",
        Provider::Custom => "",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::polish::{Destination, DestinationKind};
    use std::cell::RefCell;

    /// Replays scripted results and records whether each call was polished.
    struct Scripted {
        results: RefCell<Vec<Result<String, ProviderError>>>,
        polished_calls: RefCell<Vec<bool>>,
    }

    impl Scripted {
        fn new(results: Vec<Result<String, ProviderError>>) -> Self {
            Scripted {
                results: RefCell::new(results),
                polished_calls: RefCell::new(Vec::new()),
            }
        }
    }

    impl TranscriptionProvider for Scripted {
        fn transcribe(&self, req: &TranscriptionRequest) -> Result<String, ProviderError> {
            self.polished_calls.borrow_mut().push(req.polish.is_some());
            self.results.borrow_mut().remove(0)
        }
    }

    fn ctx() -> PolishContext {
        PolishContext {
            destination: Destination {
                app: "firefox".to_string(),
                kind: DestinationKind::Browser,
            },
        }
    }

    fn request<'a>(polish: Option<&'a PolishContext>) -> TranscriptionRequest<'a> {
        TranscriptionRequest {
            audio_path: Path::new("unused.wav"),
            model: "m",
            prompt: "",
            language: None,
            polish,
        }
    }

    fn http_500() -> ProviderError {
        ProviderError::Http {
            status: 500,
            body: "boom".to_string(),
        }
    }

    #[test]
    fn failed_polish_is_retried_verbatim_and_the_error_is_reported() {
        let provider = Scripted::new(vec![Err(http_500()), Ok("literal".to_string())]);
        let c = ctx();
        let (result, polish_error) = transcribe_with_fallback(&provider, &request(Some(&c)));
        assert_eq!(result.unwrap(), "literal");
        assert!(matches!(
            polish_error,
            Some(ProviderError::Http { status: 500, .. })
        ));
        assert_eq!(*provider.polished_calls.borrow(), vec![true, false]);
    }

    #[test]
    fn successful_polish_makes_a_single_call() {
        let provider = Scripted::new(vec![Ok("polished".to_string())]);
        let c = ctx();
        let (result, polish_error) = transcribe_with_fallback(&provider, &request(Some(&c)));
        assert_eq!(result.unwrap(), "polished");
        assert!(polish_error.is_none());
        assert_eq!(*provider.polished_calls.borrow(), vec![true]);
    }

    #[test]
    fn missing_key_is_not_retried() {
        let provider = Scripted::new(vec![Err(ProviderError::MissingApiKey)]);
        let c = ctx();
        let (result, polish_error) = transcribe_with_fallback(&provider, &request(Some(&c)));
        assert!(matches!(result, Err(ProviderError::MissingApiKey)));
        assert!(polish_error.is_none());
        assert_eq!(provider.polished_calls.borrow().len(), 1);
    }

    #[test]
    fn empty_polished_reply_is_retried() {
        let provider = Scripted::new(vec![
            Err(ProviderError::UnexpectedResponseShape("{}".to_string())),
            Ok("literal".to_string()),
        ]);
        let c = ctx();
        let (result, _) = transcribe_with_fallback(&provider, &request(Some(&c)));
        assert_eq!(result.unwrap(), "literal");
    }

    #[test]
    fn when_both_attempts_fail_the_verbatim_error_wins() {
        let provider = Scripted::new(vec![Err(http_500()), Err(ProviderError::MissingApiKey)]);
        let c = ctx();
        let (result, polish_error) = transcribe_with_fallback(&provider, &request(Some(&c)));
        assert!(matches!(result, Err(ProviderError::MissingApiKey)));
        assert!(polish_error.is_some());
    }

    #[test]
    fn verbatim_requests_are_never_retried() {
        let provider = Scripted::new(vec![Err(http_500())]);
        let (result, polish_error) = transcribe_with_fallback(&provider, &request(None));
        assert!(result.is_err());
        assert!(polish_error.is_none());
        assert_eq!(provider.polished_calls.borrow().len(), 1);
    }

    #[test]
    fn default_models_per_provider() {
        assert_eq!(
            default_model(Provider::Gemini, 10.0, 1000.0),
            "gemini-3.5-flash-lite"
        );
        assert_eq!(default_model(Provider::OpenAi, 10.0, 1000.0), "whisper-1");
        assert_eq!(default_model(Provider::Custom, 10.0, 1000.0), "");
        assert_eq!(
            default_model(Provider::Groq, 10.0, 1000.0),
            "whisper-large-v3-turbo"
        );
        assert_eq!(
            default_model(Provider::Groq, 2000.0, 1000.0),
            "whisper-large-v3"
        );
    }
}
