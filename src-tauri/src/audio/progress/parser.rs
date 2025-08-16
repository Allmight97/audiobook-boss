//! FFmpeg progress parsing utilities

/// Holds state for FFmpeg progress parsing
#[derive(Default)]
pub struct FFmpegProgressState {
    pub out_time_us: Option<i64>,
    pub total_size: Option<i64>,
    pub bitrate: Option<f64>,
    pub speed: Option<f64>,
}

/// Parses FFmpeg progress output to extract percentage or seconds of progress
pub fn parse_ffmpeg_progress(line: &str) -> Option<f32> {
    if line.contains('=') {
        let parts: Vec<&str> = line.splitn(2, '=').collect();
        if parts.len() == 2 {
            let key = parts[0].trim();
            let value = parts[1].trim();
            match key {
                "out_time_us" => {
                    if let Ok(time_us) = value.parse::<i64>() {
                        let time_seconds = time_us as f64 / 1_000_000.0;
                        return Some(time_seconds as f32);
                    }
                }
                "progress" => {
                    if value == "end" {
                        return Some(100.0);
                    } else if value == "continue" {
                        return None;
                    }
                }
                _ => {}
            }
        }
    }

    if let Some(time_str) = line.strip_prefix("time=") {
        if let Ok(duration) = parse_ffmpeg_time(time_str) {
            return Some(duration as f32);
        }
    }

    None
}

/// Parses FFmpeg time format (HH:MM:SS.ss) to seconds
fn parse_ffmpeg_time(time_str: &str) -> Result<f64, std::num::ParseFloatError> {
    let parts: Vec<&str> = time_str.split(':').collect();
    if parts.len() != 3 {
        return Ok(0.0);
    }
    let hours: f64 = parts[0].parse()?;
    let minutes: f64 = parts[1].parse()?;
    let seconds: f64 = parts[2].parse()?;
    Ok(hours * 3600.0 + minutes * 60.0 + seconds)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_ffmpeg_time() {
        assert_eq!(parse_ffmpeg_time("00:01:30.50").expect("parse time"), 90.5);
        assert_eq!(parse_ffmpeg_time("01:00:00.00").expect("parse time"), 3600.0);
    }

    #[test]
    fn test_parse_ffmpeg_progress() {
        assert_eq!(parse_ffmpeg_progress("time=00:01:30.45").expect("parse progress"), 90.45);
        assert_eq!(parse_ffmpeg_progress("out_time_us=90450000").expect("parse progress"), 90.45);
        assert_eq!(parse_ffmpeg_progress("progress=end").expect("parse progress"), 100.0);
        assert!(parse_ffmpeg_progress("progress=continue").is_none());
        assert!(parse_ffmpeg_progress("other output").is_none());
        assert_eq!(parse_ffmpeg_progress("out_time_us=1000000").expect("parse progress"), 1.0);
        assert_eq!(parse_ffmpeg_progress("out_time_us=60000000").expect("parse progress"), 60.0);
    }
}


