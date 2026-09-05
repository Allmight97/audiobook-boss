//! Accepted chapter positions use milliseconds, rounded once to nearest ms.
use crate::MetadataCoreError;
use serde::{Deserialize, Serialize};
type Result<T> = std::result::Result<T, MetadataCoreError>;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ChapterSpec {
    pub title: Option<String>,
    pub start_ms: i64,
    pub end_ms: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum CueInterpretation {
    Frames75,
    Hundredths,
}

#[derive(Debug, Clone)]
pub struct CueSheet {
    pub file_name: String,
    pub interpretation: CueInterpretation,
    pub chapters: Vec<ChapterSpec>,
}

fn invalid(line: usize, reason: &str) -> MetadataCoreError {
    MetadataCoreError::InvalidInput(format!("CUE line {line}: {reason}"))
}

#[derive(Default)]
struct Track {
    title: Option<String>,
    start: Option<(i64, i64, i64)>,
    line: usize,
}

fn timestamp(value: &str, line: usize) -> Result<(i64, i64, i64)> {
    let parts: Vec<_> = value.split(':').collect();
    if parts.len() != 3
        || parts
            .iter()
            .any(|p| p.is_empty() || !p.bytes().all(|c| c.is_ascii_digit()))
    {
        return Err(invalid(line, "expected minutes:seconds:frames"));
    }
    let values: Vec<i64> = parts
        .iter()
        .map(|p| p.parse().map_err(|_| invalid(line, "timestamp overflow")))
        .collect::<Result<_>>()?;
    if values[1] >= 60 || values[2] >= 100 {
        return Err(invalid(
            line,
            "seconds must be 0–59; final field must be 0–99",
        ));
    }
    Ok((values[0], values[1], values[2]))
}

fn quoted(value: &str, line: usize) -> Result<String> {
    let value = value.trim();
    let Some(rest) = value.strip_prefix('"') else {
        return Err(invalid(line, "expected quoted text"));
    };
    let Some(end) = rest.find('"') else {
        return Err(invalid(line, "unterminated quoted text"));
    };
    Ok(rest[..end].to_string())
}

/// Parse a single FILE/AUDIO sheet. Nonstandard fields propose hundredths;
/// the runtime/UI must obtain explicit confirmation before accepting that plan.
pub fn parse_cue(text: &str, duration_ms: i64) -> Result<CueSheet> {
    let mut file_name = None;
    let mut tracks: Vec<Track> = Vec::new();
    let mut last_number = 0;
    let mut needs_hundredths = false;
    for (index, line) in text.trim_start_matches('\u{feff}').lines().enumerate() {
        let line_number = index + 1;
        let line = line.trim();
        let (command, value) = line.split_once(char::is_whitespace).unwrap_or((line, ""));
        match command.to_ascii_uppercase().as_str() {
            "FILE" => {
                if file_name.is_some() {
                    return Err(invalid(line_number, "multiple FILE sheets are unsupported"));
                }
                let name = quoted(value, line_number)?;
                if name.is_empty() || name.contains(['/', '\\', ':']) || name == "." || name == ".."
                {
                    return Err(invalid(
                        line_number,
                        "FILE must name a local sibling, not a directory or URL",
                    ));
                }
                file_name = Some(name);
            }
            "TRACK" => {
                let fields: Vec<_> = value.split_whitespace().collect();
                let number = fields
                    .first()
                    .and_then(|v| v.parse::<u32>().ok())
                    .unwrap_or(0);
                if file_name.is_none()
                    || fields.len() != 2
                    || fields[1] != "AUDIO"
                    || number <= last_number
                {
                    return Err(invalid(
                        line_number,
                        "expected increasing AUDIO tracks after FILE",
                    ));
                }
                last_number = number;
                tracks.push(Track {
                    line: line_number,
                    ..Track::default()
                });
            }
            "TITLE" => {
                if let Some(track) = tracks.last_mut() {
                    track.title = Some(quoted(value, line_number)?);
                }
            }
            "INDEX" => {
                let fields: Vec<_> = value.split_whitespace().collect();
                if fields.len() != 2 {
                    return Err(invalid(line_number, "expected INDEX number and timestamp"));
                }
                let track = tracks
                    .last_mut()
                    .ok_or_else(|| invalid(line_number, "INDEX before TRACK"))?;
                let time = timestamp(fields[1], line_number)?;
                needs_hundredths |= time.2 >= 75;
                match fields[0] {
                    "01" if track.start.is_none() => {
                        track.start = Some(time);
                        track.line = line_number;
                    }
                    "00" => {} // pregap index is not the supplied chapter start
                    _ => {
                        return Err(invalid(
                            line_number,
                            "duplicate or unsupported INDEX; expected one INDEX 01 per track",
                        ))
                    }
                }
            }
            "PREGAP" | "POSTGAP" => {
                return Err(invalid(line_number, "explicit gaps are unsupported"))
            }
            _ => {} // REM, performer, and other descriptive tags do not change the timeline
        }
    }
    let file_name = file_name.ok_or_else(|| invalid(1, "missing FILE"))?;
    if tracks.is_empty() {
        return Err(invalid(1, "no AUDIO tracks"));
    }
    let interpretation = if needs_hundredths {
        CueInterpretation::Hundredths
    } else {
        CueInterpretation::Frames75
    };
    let denominator = match interpretation {
        CueInterpretation::Frames75 => 75,
        CueInterpretation::Hundredths => 100,
    };
    let mut chapters = Vec::new();
    for track in tracks {
        let (minutes, seconds, fraction) = track
            .start
            .ok_or_else(|| invalid(track.line, "missing INDEX 01"))?;
        let start = minutes
            .checked_mul(60)
            .and_then(|v| v.checked_add(seconds))
            .and_then(|v| v.checked_mul(denominator))
            .and_then(|v| v.checked_add(fraction))
            .and_then(|v| v.checked_mul(1000))
            .and_then(|v| v.checked_add(denominator / 2))
            .map(|v| v / denominator)
            .ok_or_else(|| invalid(track.line, "timestamp overflow"))?;
        if start >= duration_ms
            || chapters
                .last()
                .is_some_and(|c: &ChapterSpec| c.start_ms >= start)
        {
            return Err(invalid(
                track.line,
                "chapter starts must increase and precede the audio end",
            ));
        }
        if let Some(previous) = chapters.last_mut() {
            previous.end_ms = start;
        }
        chapters.push(ChapterSpec {
            title: track.title,
            start_ms: start,
            end_ms: duration_ms,
        });
    }
    validate_chapters(&chapters, duration_ms)?;
    Ok(CueSheet {
        file_name,
        interpretation,
        chapters,
    })
}

pub fn validate_chapters(chapters: &[ChapterSpec], duration_ms: i64) -> Result<()> {
    for (index, chapter) in chapters.iter().enumerate() {
        if chapter.start_ms < 0
            || chapter.end_ms <= chapter.start_ms
            || chapter.end_ms > duration_ms
            || index > 0 && chapters[index - 1].end_ms > chapter.start_ms
        {
            return Err(invalid(
                index + 1,
                "invalid, overlapping, or out-of-range chapter interval",
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cue_timestamp_interpretations_and_short_final_entry() {
        let standard = parse_cue("FILE \"book.mp3\" MP3\nTRACK 01 AUDIO\nTITLE \"Opening\"\nINDEX 01 00:00:00\nTRACK 02 AUDIO\nINDEX 01 00:13:74", 20_000).expect("standard sheet");
        assert_eq!(standard.interpretation, CueInterpretation::Frames75);
        assert_eq!(standard.chapters[1].start_ms, 13_987);
        assert_eq!(standard.chapters[0].end_ms, 13_987);
        let pregap = parse_cue(
            "FILE \"book.mp3\" MP3\nTRACK 01 AUDIO\nINDEX 00 00:00:94\nINDEX 01 00:01:50",
            20_000,
        )
        .expect("whole-sheet interpretation includes pregap timestamps");
        assert_eq!(pregap.interpretation, CueInterpretation::Hundredths);
        assert_eq!(pregap.chapters[0].start_ms, 1_500);
        let nonstandard = parse_cue("FILE \"Lost Time.mp3\" MP3\nTRACK 01 AUDIO\nTITLE \"Chapter 01\"\nINDEX 01 00:00:00\nTRACK 02 AUDIO\nTITLE \"Chapter 02\"\nINDEX 01 00:13:94\nTRACK 42 AUDIO\nTITLE \"Chapter 42\"\nINDEX 01 580:52:68", 34_852_687).expect("hundredths proposal");
        assert_eq!(nonstandard.interpretation, CueInterpretation::Hundredths);
        assert_eq!(nonstandard.chapters[1].start_ms, 13_940);
        assert_eq!(
            nonstandard.chapters[2],
            ChapterSpec {
                title: Some("Chapter 42".into()),
                start_ms: 34_852_680,
                end_ms: 34_852_687
            }
        );
    }
    #[test]
    fn cue_rejects_unsafe_ambiguous_and_invalid_sheets() {
        for text in ["FILE \"../book.mp3\" MP3", "FILE \"a.mp3\" MP3\nFILE \"b.mp3\" MP3", "FILE \"a.mp3\" MP3\nTRACK 01 AUDIO", "FILE \"a.mp3\" MP3\nTRACK 01 AUDIO\nINDEX 01 00:60:00", "FILE \"a.mp3\" MP3\nTRACK 01 AUDIO\nINDEX 01 00:00:100", "FILE \"a.mp3\" MP3\nTRACK 01 AUDIO\nINDEX 01 00:00:00\nINDEX 01 00:01:00", "FILE \"a.mp3\" MP3\nTRACK 01 AUDIO\nINDEX 01 00:02:00\nTRACK 02 AUDIO\nINDEX 01 00:01:00", "FILE \"a.mp3\" MP3\nTRACK 01 AUDIO\nINDEX 01 00:20:00"] {
            assert!(parse_cue(text, 20_000).expect_err(text).to_string().contains("CUE line"));
        }
    }
}
