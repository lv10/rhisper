// gemini_provider_test.rs - integration tests for GeminiProvider against a
// local mock HTTP server. Unlike provider_test.rs, the mock hands the raw
// request back over a channel so tests can assert on the path, headers and
// JSON body. No network access or real key is needed.

use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::Duration;

use rhisper_core::provider::{
    GeminiProvider, ProviderError, TranscriptionProvider, TranscriptionRequest,
};

const OK_BODY: &str =
    r#"{"candidates":[{"content":{"parts":[{"text":" hello world"}]},"finishReason":"STOP"}]}"#;

/// Starts a one-shot mock server that replies with `status_line` + `body`
/// and sends the full raw request text it received to the returned channel.
fn spawn_mock_server(status_line: &'static str, body: &'static str) -> (String, Receiver<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind mock server");
    let addr = listener.local_addr().unwrap();
    let (tx, rx) = mpsc::channel();

    thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            stream
                .set_read_timeout(Some(Duration::from_millis(300)))
                .ok();
            let mut received = Vec::new();
            let mut buf = [0u8; 8192];
            loop {
                match stream.read(&mut buf) {
                    Ok(0) => break,
                    Ok(n) => received.extend_from_slice(&buf[..n]),
                    Err(_) => break, // read timeout: client has finished sending
                }
            }
            let _ = tx.send(String::from_utf8_lossy(&received).into_owned());
            let response = format!(
                "{status_line}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            let _ = stream.write_all(response.as_bytes());
            let _ = stream.flush();
        }
    });

    (format!("http://{addr}"), rx)
}

fn temp_audio_file_of_size(len: usize) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "rhisper-gemini-test-{}-{}.wav",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let mut bytes = b"RIFF....WAVEfmt ".to_vec();
    bytes.resize(len.max(bytes.len()), 0);
    std::fs::write(&path, bytes).unwrap();
    path
}

fn temp_audio_file() -> PathBuf {
    temp_audio_file_of_size(0)
}

fn provider_for(base_url: String) -> GeminiProvider {
    GeminiProvider {
        base_url,
        api_key: "test-key".to_string(),
    }
}

fn request_with<'a>(
    audio_path: &'a std::path::Path,
    model: &'a str,
    prompt: &'a str,
    language: Option<&'a str>,
) -> TranscriptionRequest<'a> {
    TranscriptionRequest {
        audio_path,
        model,
        prompt,
        language,
    }
}

fn request(audio_path: &std::path::Path) -> TranscriptionRequest<'_> {
    request_with(audio_path, "gemini-test-model", "", None)
}

/// Splits a captured raw request into (head, parsed JSON body).
fn split_request(raw: &str) -> (String, serde_json::Value) {
    let (head, body) = raw.split_once("\r\n\r\n").expect("request has a body");
    let json = serde_json::from_str(body).expect("request body is JSON");
    (head.to_string(), json)
}

/// The user-turn text part (index 0), where language and vocabulary go.
fn user_text(body: &serde_json::Value) -> String {
    body["contents"][0]["parts"][0]["text"]
        .as_str()
        .unwrap_or_default()
        .to_string()
}

fn transcribe_via_mock(
    status_line: &'static str,
    body: &'static str,
) -> Result<String, ProviderError> {
    let (base_url, _rx) = spawn_mock_server(status_line, body);
    let audio = temp_audio_file();
    let result = provider_for(base_url).transcribe(&request(&audio));
    let _ = std::fs::remove_file(&audio);
    result
}

// --- US1: success path -----------------------------------------------------

#[test]
fn success_response_is_parsed_and_trimmed() {
    let result = transcribe_via_mock("HTTP/1.1 200 OK", OK_BODY);
    assert_eq!(result.unwrap(), "hello world");
}

#[test]
fn request_has_expected_path_header_and_body_shape() {
    let (base_url, rx) = spawn_mock_server("HTTP/1.1 200 OK", OK_BODY);
    let audio = temp_audio_file();
    provider_for(base_url).transcribe(&request(&audio)).unwrap();
    let _ = std::fs::remove_file(&audio);

    let raw = rx.recv_timeout(Duration::from_secs(2)).unwrap();
    let (head, body) = split_request(&raw);
    let head_lower = head.to_ascii_lowercase();

    assert!(
        head.starts_with("POST /models/gemini-test-model:generateContent "),
        "unexpected request line: {}",
        head.lines().next().unwrap_or("")
    );
    assert!(head_lower.contains("x-goog-api-key: test-key"));
    assert!(!head.lines().next().unwrap().contains("test-key"));

    assert!(body["system_instruction"]["parts"][0]["text"]
        .as_str()
        .is_some_and(|t| !t.is_empty()));
    let inline = &body["contents"][0]["parts"][1]["inline_data"];
    assert_eq!(inline["mime_type"], "audio/wav");
    assert!(inline["data"].as_str().is_some_and(|d| !d.is_empty()));
    assert_eq!(body["generationConfig"]["temperature"], 0);
}

#[test]
fn multiple_text_parts_are_concatenated_and_trimmed() {
    let result = transcribe_via_mock(
        "HTTP/1.1 200 OK",
        r#"{"candidates":[{"content":{"parts":[{"text":" hello "},{"text":"world "}]},"finishReason":"STOP"}]}"#,
    );
    assert_eq!(result.unwrap(), "hello world");
}

// --- US2: failure paths ----------------------------------------------------

#[test]
fn missing_api_key_is_rejected_before_any_network_call() {
    let provider = GeminiProvider {
        base_url: "http://127.0.0.1:1".to_string(),
        api_key: String::new(),
    };
    let audio = temp_audio_file();
    let result = provider.transcribe(&request(&audio));
    let _ = std::fs::remove_file(&audio);
    assert!(matches!(result, Err(ProviderError::MissingApiKey)));
}

#[test]
fn http_error_status_never_produces_a_transcript() {
    let result = transcribe_via_mock(
        "HTTP/1.1 403 Forbidden",
        r#"{"error":{"code":403,"message":"API key not valid","status":"PERMISSION_DENIED"}}"#,
    );
    match result {
        Err(ProviderError::Http { status, body }) => {
            assert_eq!(status, 403);
            assert!(body.contains("PERMISSION_DENIED"));
        }
        other => panic!("expected Http error, got {other:?}"),
    }
}

#[test]
fn malformed_json_body_is_reported_not_swallowed() {
    let result = transcribe_via_mock("HTTP/1.1 200 OK", "{ not json");
    assert!(matches!(
        result,
        Err(ProviderError::UnexpectedResponseShape(_))
    ));
}

#[test]
fn non_json_body_is_reported() {
    let result = transcribe_via_mock("HTTP/1.1 200 OK", "<html>gateway</html>");
    assert!(matches!(
        result,
        Err(ProviderError::UnexpectedResponseShape(_))
    ));
}

#[test]
fn blocked_prompt_is_an_error() {
    let result = transcribe_via_mock(
        "HTTP/1.1 200 OK",
        r#"{"promptFeedback":{"blockReason":"SAFETY"}}"#,
    );
    assert!(matches!(
        result,
        Err(ProviderError::UnexpectedResponseShape(_))
    ));
}

#[test]
fn non_stop_finish_reason_does_not_return_partial_text() {
    let result = transcribe_via_mock(
        "HTTP/1.1 200 OK",
        r#"{"candidates":[{"content":{"parts":[{"text":"partial"}]},"finishReason":"MAX_TOKENS"}]}"#,
    );
    assert!(matches!(
        result,
        Err(ProviderError::UnexpectedResponseShape(_))
    ));
}

#[test]
fn missing_finish_reason_is_accepted() {
    let result = transcribe_via_mock(
        "HTTP/1.1 200 OK",
        r#"{"candidates":[{"content":{"parts":[{"text":"ok"}]}}]}"#,
    );
    assert_eq!(result.unwrap(), "ok");
}

#[test]
fn empty_results_are_errors() {
    for body in [
        r#"{}"#,
        r#"{"candidates":[]}"#,
        r#"{"candidates":[{"content":{"parts":[]},"finishReason":"STOP"}]}"#,
        r#"{"candidates":[{"content":{"parts":[{"text":"   "}]},"finishReason":"STOP"}]}"#,
    ] {
        let result = transcribe_via_mock("HTTP/1.1 200 OK", body);
        assert!(
            matches!(result, Err(ProviderError::UnexpectedResponseShape(_))),
            "body {body} should be rejected, got {result:?}"
        );
    }
}

#[test]
fn network_unreachable_is_a_network_error() {
    let provider = provider_for("http://127.0.0.1:1".to_string());
    let audio = temp_audio_file();
    let result = provider.transcribe(&request(&audio));
    let _ = std::fs::remove_file(&audio);
    assert!(matches!(result, Err(ProviderError::Network(_))));
}

#[test]
fn oversized_audio_is_rejected_without_a_network_attempt() {
    // Unreachable base URL: a network attempt would yield Network, not
    // UnexpectedResponseShape.
    let provider = provider_for("http://127.0.0.1:1".to_string());
    let audio = temp_audio_file_of_size(14 * 1024 * 1024 + 1);
    let result = provider.transcribe(&request(&audio));
    let _ = std::fs::remove_file(&audio);
    match result {
        Err(ProviderError::UnexpectedResponseShape(msg)) => {
            assert!(msg.contains("too large"), "unexpected message: {msg}")
        }
        other => panic!("expected size rejection, got {other:?}"),
    }
}

#[test]
fn api_key_never_appears_in_error_text() {
    let result = transcribe_via_mock("HTTP/1.1 500 Internal Server Error", "{}");
    let text = result.unwrap_err().to_string();
    assert!(!text.contains("test-key"));
}

// --- US3: language, model, prompt -----------------------------------------

fn captured_body_for(
    model: &'static str,
    prompt: &'static str,
    language: Option<&'static str>,
) -> (String, serde_json::Value) {
    let (base_url, rx) = spawn_mock_server("HTTP/1.1 200 OK", OK_BODY);
    let audio = temp_audio_file();
    provider_for(base_url)
        .transcribe(&request_with(&audio, model, prompt, language))
        .unwrap();
    let _ = std::fs::remove_file(&audio);
    let raw = rx.recv_timeout(Duration::from_secs(2)).unwrap();
    let (head, body) = split_request(&raw);
    (head, body)
}

#[test]
fn language_hint_is_included_only_when_set() {
    let (_, with_lang) = captured_body_for("m", "", Some("de"));
    assert!(user_text(&with_lang).contains("de"));

    let (_, without) = captured_body_for("m", "", None);
    assert!(!user_text(&without)
        .to_ascii_lowercase()
        .contains("language code"));
}

#[test]
fn model_override_is_used_in_the_request_path() {
    let (head, _) = captured_body_for("gemini-custom-x", "", None);
    assert!(head.starts_with("POST /models/gemini-custom-x:generateContent "));
}

#[test]
fn transcription_prompt_is_passed_as_vocabulary_guidance() {
    let (_, with_prompt) = captured_body_for("m", "rhisper, Groq", None);
    assert!(user_text(&with_prompt).contains("rhisper, Groq"));

    let (_, empty_prompt) = captured_body_for("m", "", None);
    assert!(!user_text(&empty_prompt).contains("rhisper, Groq"));
}
