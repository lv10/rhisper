// config.rs - rhisperrc loading, defaults, and first-run bootstrap.
//
// A hand-rolled `key : value` parser and its hardcoded defaults. The flat
// format is kept intentionally simple (not TOML) since it doesn't need
// nested structures or arrays beyond what's already handled.

use crate::placeholder::Setting as PlaceholderSetting;
use std::env;
use std::fs;
use std::io;
use std::path::PathBuf;

/// The canonical default config, embedded at compile time. This is both
/// what Config::default() mirrors and what gets written out verbatim on a
/// non-interactive first run, so there's a single source of truth that
/// doesn't depend on locating an installed template at an unknown prefix.
pub const DEFAULT_RHISPERRC: &str = include_str!("../default_rhisperrc");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PasteMode {
    Type,
    Clipboard,
    ClipboardRestore,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Provider {
    Groq,
    OpenAi,
    Custom,
}

#[derive(Debug, Clone)]
pub struct Config {
    pub long_recording_threshold: f64,
    pub transcription_prompt: String,
    pub language: String,
    pub paste_mode: PasteMode,
    pub non_ascii_initial_delay: f64,
    pub non_ascii_default_delay: f64,
    pub keyboard_layout: String,
    pub silence_threshold: f64,
    pub min_speech_seconds: f64,
    pub placeholders: PlaceholderSetting,
    pub placeholder_recording: String,
    pub placeholder_transcribing: String,
    pub placeholder_silent: String,
    pub provider: Provider,
    pub api_base_url: String,
    pub model: String,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            long_recording_threshold: 1000.0,
            transcription_prompt: String::new(),
            language: String::new(),
            paste_mode: PasteMode::Type,
            non_ascii_initial_delay: 0.15,
            non_ascii_default_delay: 0.025,
            keyboard_layout: "us".to_string(),
            silence_threshold: -50.0,
            min_speech_seconds: 0.3,
            placeholders: PlaceholderSetting::Auto,
            placeholder_recording: "(recording...)".to_string(),
            placeholder_transcribing: "(transcribing...)".to_string(),
            placeholder_silent: "(no sound detected)".to_string(),
            provider: Provider::Groq,
            api_base_url: String::new(),
            model: String::new(),
        }
    }
}

pub fn config_dir() -> PathBuf {
    let base = env::var("XDG_CONFIG_HOME")
        .ok()
        .filter(|p| !p.is_empty())
        .unwrap_or_else(|| format!("{}/.config", env::var("HOME").unwrap_or_default()));
    PathBuf::from(base).join("rhisper")
}

pub fn config_path() -> PathBuf {
    config_dir().join("rhisperrc")
}

/// Trim whitespace, then one layer of surrounding double quotes.
fn trim_value(raw: &str) -> String {
    let v = raw.trim();
    let v = v.strip_prefix('"').unwrap_or(v);
    let v = v.strip_suffix('"').unwrap_or(v);
    v.to_string()
}

fn parse_f64(value: &str, fallback: f64) -> f64 {
    value.trim().parse().unwrap_or(fallback)
}

/// Parses the rhisperrc key:value format into a Config, starting from
/// defaults for any key that's absent, malformed, or unrecognized -
/// unrecognized keys are silently ignored, exactly as the original bash
/// `case` statement does.
pub fn parse(contents: &str) -> Config {
    let mut config = Config::default();

    for line in contents.lines() {
        let trimmed_line = line.trim_start();
        if trimmed_line.starts_with('#') || trimmed_line.is_empty() {
            continue;
        }

        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        let key = key.trim();
        let value = trim_value(value);

        match key {
            "long-recording-threshold" => {
                config.long_recording_threshold = parse_f64(&value, config.long_recording_threshold)
            }
            "transcription-prompt" => config.transcription_prompt = value,
            "language" => config.language = value,
            "paste-mode" => {
                config.paste_mode = match value.as_str() {
                    "clipboard" => PasteMode::Clipboard,
                    "clipboard-restore" => PasteMode::ClipboardRestore,
                    _ => PasteMode::Type,
                }
            }
            "non-ascii-initial-delay" => {
                config.non_ascii_initial_delay = parse_f64(&value, config.non_ascii_initial_delay)
            }
            "non-ascii-default-delay" => {
                config.non_ascii_default_delay = parse_f64(&value, config.non_ascii_default_delay)
            }
            "keyboard-layout" => config.keyboard_layout = value,
            "silence-threshold" => {
                config.silence_threshold = parse_f64(&value, config.silence_threshold)
            }
            "min-speech-seconds" => {
                config.min_speech_seconds = parse_f64(&value, config.min_speech_seconds)
            }
            "placeholders" => {
                config.placeholders = match value.as_str() {
                    "inline" => PlaceholderSetting::Inline,
                    "notify" => PlaceholderSetting::Notify,
                    "off" => PlaceholderSetting::Off,
                    _ => PlaceholderSetting::Auto,
                }
            }
            "placeholder-recording" => config.placeholder_recording = value,
            "placeholder-transcribing" => config.placeholder_transcribing = value,
            "placeholder-silent" => config.placeholder_silent = value,
            "provider" => {
                config.provider = match value.as_str() {
                    "openai" => Provider::OpenAi,
                    "custom" => Provider::Custom,
                    _ => Provider::Groq,
                }
            }
            "api-base-url" => config.api_base_url = value,
            "model" => config.model = value,
            _ => {}
        }
    }

    config
}

/// Loads the user's config, bootstrapping a fresh one non-interactively
/// (copying the embedded default template) if none exists yet. Safe to
/// call from a package post-install hook or a bare first invocation - no
/// interactive prompt is involved.
pub fn load_or_bootstrap() -> io::Result<Config> {
    let path = config_path();

    if !path.exists() {
        fs::create_dir_all(config_dir())?;
        fs::write(&path, DEFAULT_RHISPERRC)?;
    }

    let contents = fs::read_to_string(&path)?;
    Ok(parse(&contents))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_embedded_template() {
        let from_template = parse(DEFAULT_RHISPERRC);
        let defaults = Config::default();
        assert_eq!(
            from_template.long_recording_threshold,
            defaults.long_recording_threshold
        );
        assert_eq!(from_template.paste_mode, defaults.paste_mode);
        assert_eq!(from_template.keyboard_layout, defaults.keyboard_layout);
        assert_eq!(from_template.silence_threshold, defaults.silence_threshold);
        assert_eq!(
            from_template.min_speech_seconds,
            defaults.min_speech_seconds
        );
        assert_eq!(from_template.placeholders, defaults.placeholders);
        assert_eq!(
            from_template.placeholder_recording,
            defaults.placeholder_recording
        );
    }

    #[test]
    fn parses_all_known_keys() {
        let contents = r#"
# a comment
long-recording-threshold : 42
transcription-prompt     : "hello world"
language                 : "de"
paste-mode               : "clipboard-restore"
non-ascii-initial-delay : 0.5
non-ascii-default-delay : 0.1
keyboard-layout : dk
silence-threshold  : -30
min-speech-seconds : 0.5
placeholders : notify
placeholder-recording : "MIC"
placeholder-transcribing : "..."
placeholder-silent : "quiet"
provider : openai
api-base-url : "https://example.com/v1"
model : "whisper-1"
"#;
        let c = parse(contents);
        assert_eq!(c.long_recording_threshold, 42.0);
        assert_eq!(c.transcription_prompt, "hello world");
        assert_eq!(c.language, "de");
        assert_eq!(c.paste_mode, PasteMode::ClipboardRestore);
        assert_eq!(c.non_ascii_initial_delay, 0.5);
        assert_eq!(c.non_ascii_default_delay, 0.1);
        assert_eq!(c.keyboard_layout, "dk");
        assert_eq!(c.silence_threshold, -30.0);
        assert_eq!(c.min_speech_seconds, 0.5);
        assert_eq!(c.placeholders, PlaceholderSetting::Notify);
        assert_eq!(c.placeholder_recording, "MIC");
        assert_eq!(c.placeholder_transcribing, "...");
        assert_eq!(c.placeholder_silent, "quiet");
        assert_eq!(c.provider, Provider::OpenAi);
        assert_eq!(c.api_base_url, "https://example.com/v1");
        assert_eq!(c.model, "whisper-1");
    }

    #[test]
    fn unknown_keys_and_malformed_lines_are_ignored() {
        let contents = "not-a-real-key: 5\nno-colon-here\nkeyboard-layout: dk\n";
        let c = parse(contents);
        assert_eq!(c.keyboard_layout, "dk");
    }

    #[test]
    fn blank_and_comment_lines_are_skipped() {
        let contents = "\n  # comment\nkeyboard-layout: dk\n";
        let c = parse(contents);
        assert_eq!(c.keyboard_layout, "dk");
    }
}
