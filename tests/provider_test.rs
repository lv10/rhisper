// provider_test.rs - integration tests for the transcription provider
// against a local mock HTTP server, covering the failure modes the
// original transcribe() didn't handle: HTTP errors, malformed JSON, and
// network-unreachable. None of these should ever produce the literal
// string "null" or a silent empty result - every path returns a typed
// ProviderError.

use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::PathBuf;
use std::thread;
use std::time::Duration;

use rhisper_core::provider::{
    OpenAiCompatibleProvider, ProviderError, TranscriptionProvider, TranscriptionRequest,
};

/// Starts a one-shot mock HTTP server that reads (and discards) a full
/// request, then replies with `status_line` + `body`, computing
/// Content-Length itself so callers can't get the byte count wrong. Read
/// timeout is used instead of Content-Length parsing on the request side to
/// detect "client has finished sending" - fine for these small payloads.
fn spawn_mock_server(status_line: &'static str, body: &'static str) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind mock server");
    let addr = listener.local_addr().unwrap();

    thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            stream
                .set_read_timeout(Some(Duration::from_millis(300)))
                .ok();
            let mut buf = [0u8; 8192];
            loop {
                match stream.read(&mut buf) {
                    Ok(0) => break,
                    Ok(_) => continue,
                    Err(_) => break, // read timeout: client has finished sending
                }
            }
            let response = format!(
                "{status_line}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            let _ = stream.write_all(response.as_bytes());
            let _ = stream.flush();
        }
    });

    format!("http://{addr}")
}

/// A tiny placeholder "audio" file - the mock server never inspects the
/// multipart body, so any bytes suffice.
fn temp_audio_file() -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "rhisper-provider-test-{}-{}.wav",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::write(&path, b"RIFF....WAVEfmt ").unwrap();
    path
}

fn provider_for(base_url: String) -> OpenAiCompatibleProvider {
    OpenAiCompatibleProvider {
        base_url,
        api_key: "test-key".to_string(),
    }
}

fn request(audio_path: &std::path::Path) -> TranscriptionRequest<'_> {
    TranscriptionRequest {
        audio_path,
        model: "whisper-large-v3-turbo",
        prompt: "",
        language: None,
    }
}

#[test]
fn success_response_is_parsed_and_leading_space_trimmed() {
    let base_url = spawn_mock_server("HTTP/1.1 200 OK", "{\"text\": \" hello world\"}");
    let provider = provider_for(base_url);
    let audio = temp_audio_file();

    let result = provider.transcribe(&request(&audio));

    std::fs::remove_file(&audio).ok();
    assert_eq!(result.unwrap(), "hello world");
}

#[test]
fn http_error_status_never_produces_a_transcript() {
    let base_url = spawn_mock_server(
        "HTTP/1.1 401 Unauthorized",
        "{\"error\":\"invalid api key\"}",
    );
    let provider = provider_for(base_url);
    let audio = temp_audio_file();

    let result = provider.transcribe(&request(&audio));

    std::fs::remove_file(&audio).ok();
    match result {
        Err(ProviderError::Http { status, body }) => {
            assert_eq!(status, 401);
            assert!(body.contains("invalid api key"));
        }
        other => panic!("expected Http error, got {other:?}"),
    }
}

#[test]
fn malformed_json_body_is_reported_not_swallowed() {
    let base_url = spawn_mock_server("HTTP/1.1 200 OK", "not valid json at all");
    let provider = provider_for(base_url);
    let audio = temp_audio_file();

    let result = provider.transcribe(&request(&audio));

    std::fs::remove_file(&audio).ok();
    match result {
        Err(ProviderError::UnexpectedResponseShape(body)) => {
            assert!(body.contains("not valid json"));
        }
        other => panic!("expected UnexpectedResponseShape error, got {other:?}"),
    }
}

#[test]
fn response_missing_text_field_is_reported() {
    let base_url = spawn_mock_server("HTTP/1.1 200 OK", "{\"other\": true}");
    let provider = provider_for(base_url);
    let audio = temp_audio_file();

    let result = provider.transcribe(&request(&audio));

    std::fs::remove_file(&audio).ok();
    assert!(matches!(
        result,
        Err(ProviderError::UnexpectedResponseShape(_))
    ));
}

#[test]
fn network_unreachable_is_a_network_error_not_a_silent_failure() {
    // Nothing is listening on this port: connection should be refused
    // immediately rather than hanging the test suite.
    let provider = provider_for("http://127.0.0.1:1".to_string());
    let audio = temp_audio_file();

    let result = provider.transcribe(&request(&audio));

    std::fs::remove_file(&audio).ok();
    assert!(matches!(result, Err(ProviderError::Network(_))));
}

#[test]
fn missing_api_key_is_rejected_before_any_network_call() {
    let provider = OpenAiCompatibleProvider {
        base_url: "http://127.0.0.1:1".to_string(),
        api_key: String::new(),
    };
    let audio = temp_audio_file();

    let result = provider.transcribe(&request(&audio));

    std::fs::remove_file(&audio).ok();
    assert!(matches!(result, Err(ProviderError::MissingApiKey)));
}
