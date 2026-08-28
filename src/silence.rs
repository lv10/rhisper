// silence.rs - silence detection for recorded audio.
//
// Uses `ffmpeg -af silencedetect`, which reports individual silent
// intervals, so silence-percentage can be applied precisely: a recording
// only counts as silent once the fraction of its duration spent below
// silence-threshold reaches silence-percentage. All math is in-process.

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
    pub silent_percentage: f64,
}

/// Parses ffmpeg's silencedetect stderr output into total silent seconds.
/// Lines look like:
///   [silencedetect @ 0x...] silence_start: 0.123
///   [silencedetect @ 0x...] silence_end: 1.456 | silence_duration: 1.333
/// A trailing silence_start with no matching silence_end (the recording
/// ended while still silent) is credited through to end-of-clip, since
/// ffmpeg itself never emits a duration for that final open interval.
fn parse_silent_seconds(stderr: &str, total_duration: f64) -> f64 {
    let mut silent_seconds = 0.0;
    let mut pending_start: Option<f64> = None;

    for line in stderr.lines() {
        if let Some(idx) = line.find("silence_duration:") {
            let rest = &line[idx + "silence_duration:".len()..];
            if let Some(value) = rest.split_whitespace().next() {
                if let Ok(d) = value.parse::<f64>() {
                    silent_seconds += d;
                }
            }
            pending_start = None;
        } else if let Some(idx) = line.find("silence_start:") {
            let rest = &line[idx + "silence_start:".len()..];
            if let Some(value) = rest.split_whitespace().next() {
                pending_start = value.parse::<f64>().ok();
            }
        }
    }

    if let Some(start) = pending_start {
        silent_seconds += (total_duration - start).max(0.0);
    }

    silent_seconds
}

pub fn analyze(recording: &str, threshold_db: f64, required_percentage: f64) -> SilenceReport {
    let duration = get_duration(recording);
    if duration <= 0.0 {
        return SilenceReport {
            is_silent: false,
            silent_percentage: 0.0,
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
                silent_percentage: 0.0,
            }
        }
    };

    let silent_seconds = parse_silent_seconds(&stderr, duration);
    let silent_percentage = ((silent_seconds / duration) * 100.0).min(100.0);

    SilenceReport {
        is_silent: silent_percentage >= required_percentage,
        silent_percentage,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fully_silent_clip_hits_100_percent() {
        let stderr = "[silencedetect @ 0x1] silence_start: 0\n\
                       [silencedetect @ 0x1] silence_end: 5.0 | silence_duration: 5.0\n";
        assert_eq!(parse_silent_seconds(stderr, 5.0), 5.0);
    }

    #[test]
    fn partially_silent_clip_computes_correct_seconds() {
        let stderr = "[silencedetect @ 0x1] silence_start: 0\n\
                       [silencedetect @ 0x1] silence_end: 2.0 | silence_duration: 2.0\n";
        // 2s silent out of a 10s clip
        assert_eq!(parse_silent_seconds(stderr, 10.0), 2.0);
    }

    #[test]
    fn trailing_open_silence_interval_counts_to_end_of_clip() {
        let stderr = "[silencedetect @ 0x1] silence_start: 8.0\n";
        // No matching silence_end: recording ended while still silent.
        assert_eq!(parse_silent_seconds(stderr, 10.0), 2.0);
    }

    #[test]
    fn no_silence_detected_is_zero() {
        assert_eq!(parse_silent_seconds("", 10.0), 0.0);
    }
}
