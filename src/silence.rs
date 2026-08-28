// silence.rs - silence detection for recorded audio.
//
// Uses `ffmpeg -af silencedetect`, which reports individual silent
// intervals, to find the longest contiguous stretch of non-silent audio
// anywhere in the recording. A recording only counts as silent if that
// stretch never reaches min-speech-seconds - this is deliberately an
// absolute duration, not a percentage of the whole clip, since how long
// someone pauses before or after speaking has no bearing on whether they
// said something. All math is in-process.

use std::process::Command;

pub fn get_duration(recording: &str) -> f64 {
    let output = Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-show_entries",
            "format=duration",
            "-of",
            "default=noprint_wrappers=1:nokey=1",
            recording,
        ])
        .output();

    match output {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout)
            .trim()
            .parse()
            .unwrap_or(0.0),
        _ => 0.0,
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SilenceReport {
    pub is_silent: bool,
    pub longest_active_seconds: f64,
}

/// Parses ffmpeg's silencedetect stderr output into a list of silent
/// (start, end) intervals. Lines look like:
///   [silencedetect @ 0x...] silence_start: 0.123
///   [silencedetect @ 0x...] silence_end: 1.456 | silence_duration: 1.333
/// A trailing silence_start with no matching silence_end (the recording
/// ended while still silent) is closed at total_duration, since ffmpeg
/// itself never emits an end for that final open interval.
fn parse_silent_intervals(stderr: &str, total_duration: f64) -> Vec<(f64, f64)> {
    let mut intervals = Vec::new();
    let mut pending_start: Option<f64> = None;

    for line in stderr.lines() {
        if let Some(idx) = line.find("silence_start:") {
            let rest = &line[idx + "silence_start:".len()..];
            pending_start = rest.split_whitespace().next().and_then(|v| v.parse().ok());
        } else if let Some(idx) = line.find("silence_end:") {
            let rest = &line[idx + "silence_end:".len()..];
            if let Some(end) = rest.split_whitespace().next().and_then(|v| v.parse().ok()) {
                if let Some(start) = pending_start {
                    intervals.push((start, end));
                }
            }
            pending_start = None;
        }
    }

    if let Some(start) = pending_start {
        intervals.push((start, total_duration));
    }

    intervals
}

/// Returns the longest contiguous stretch of non-silent audio - the gaps
/// before, between, and after the silent intervals. This answers "did the
/// user say something" without being skewed by how much silent padding
/// surrounds it, unlike a percentage of the whole clip's duration.
fn longest_active_stretch(stderr: &str, total_duration: f64) -> f64 {
    let intervals = parse_silent_intervals(stderr, total_duration);

    let mut longest = 0.0f64;
    let mut cursor = 0.0f64;

    for (start, end) in &intervals {
        longest = longest.max(start - cursor);
        cursor = cursor.max(*end);
    }
    longest = longest.max(total_duration - cursor);

    longest.max(0.0)
}

pub fn analyze(recording: &str, threshold_db: f64, min_speech_seconds: f64) -> SilenceReport {
    let duration = get_duration(recording);
    if duration <= 0.0 {
        // Can't tell anything about a recording we can't measure - fail
        // open (not silent) rather than block transcription.
        return SilenceReport {
            is_silent: false,
            longest_active_seconds: 0.0,
        };
    }

    let output = Command::new("ffmpeg")
        .args([
            "-i",
            recording,
            "-af",
            &format!("silencedetect=noise={threshold_db}dB:d=0.1"),
            "-f",
            "null",
            "-",
        ])
        .output();

    let stderr = match output {
        Ok(o) => String::from_utf8_lossy(&o.stderr).into_owned(),
        Err(_) => {
            return SilenceReport {
                is_silent: false,
                longest_active_seconds: duration,
            }
        }
    };

    let longest_active_seconds = longest_active_stretch(&stderr, duration);

    SilenceReport {
        is_silent: longest_active_seconds < min_speech_seconds,
        longest_active_seconds,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fully_silent_clip_has_zero_active_stretch() {
        let stderr = "[silencedetect @ 0x1] silence_start: 0\n\
                       [silencedetect @ 0x1] silence_end: 5.0 | silence_duration: 5.0\n";
        assert_eq!(longest_active_stretch(stderr, 5.0), 0.0);
    }

    #[test]
    fn leading_silence_leaves_trailing_active_stretch() {
        let stderr = "[silencedetect @ 0x1] silence_start: 0\n\
                       [silencedetect @ 0x1] silence_end: 2.0 | silence_duration: 2.0\n";
        // Silent 0-2s in a 10s clip: the remaining 8s (2-10) is active.
        assert_eq!(longest_active_stretch(stderr, 10.0), 8.0);
    }

    #[test]
    fn trailing_open_silence_interval_closes_at_end_of_clip() {
        let stderr = "[silencedetect @ 0x1] silence_start: 8.0\n";
        // No matching silence_end: recording ended while still silent.
        // Active stretch is the 8s before the silence started.
        assert_eq!(longest_active_stretch(stderr, 10.0), 8.0);
    }

    #[test]
    fn no_silence_detected_means_whole_clip_is_active() {
        assert_eq!(longest_active_stretch("", 10.0), 10.0);
    }

    #[test]
    fn speech_surrounded_by_padding_is_not_penalized_for_pause_length() {
        // The exact real-world shape that caused the false-positive bug:
        // silence, then a short speech burst, then silence again. The old
        // percentage-of-clip metric would count both paddings against the
        // user; the active-stretch metric only cares about the gap between
        // them, regardless of how long the paddings are.
        let stderr = "[silencedetect @ 0x1] silence_start: 0\n\
                       [silencedetect @ 0x1] silence_end: 2.0 | silence_duration: 2.0\n\
                       [silencedetect @ 0x1] silence_start: 2.3\n\
                       [silencedetect @ 0x1] silence_end: 5.8 | silence_duration: 3.5\n";
        let active = longest_active_stretch(stderr, 6.0);
        assert!(
            (active - 0.3).abs() < 1e-9,
            "expected 0.3s active, got {active}"
        );
    }

    #[test]
    fn min_speech_seconds_gates_is_silent_correctly() {
        let stderr = "[silencedetect @ 0x1] silence_start: 0\n\
                       [silencedetect @ 0x1] silence_end: 2.0 | silence_duration: 2.0\n\
                       [silencedetect @ 0x1] silence_start: 2.3\n\
                       [silencedetect @ 0x1] silence_end: 5.8 | silence_duration: 3.5\n";
        let active = longest_active_stretch(stderr, 6.0);
        // A lenient threshold accepts the 0.3s burst as real speech...
        assert!(active >= 0.2);
        // ...but a stricter one correctly still calls it too short.
        assert!(active < 0.5);
    }
}
