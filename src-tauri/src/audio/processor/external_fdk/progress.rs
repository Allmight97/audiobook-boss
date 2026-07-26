use crate::processing::ProgressEmitter;

pub(super) fn emit_external_progress(
    ui: &ProgressEmitter,
    line: &str,
    total_ms: f64,
    current_file: Option<String>,
) {
    if let Some(progress_ms) = parse_progress_ms(line) {
        let percentage = ((progress_ms / total_ms) * 89.0) as f32;
        ui.emit_converting_progress(
            percentage.clamp(1.0, 89.0),
            "Encoding with external FDK AAC...",
            current_file,
            None,
        );
    }
}

pub(super) fn parse_progress_ms(line: &str) -> Option<f64> {
    let (_, raw) = line.split_once('=')?;
    if line.starts_with("out_time_ms=") {
        // FFmpeg keeps this legacy key name, but current builds report the same
        // microsecond value as out_time_us.
        return raw.parse::<f64>().ok().map(|value| value / 1000.0);
    }

    if line.starts_with("out_time_us=") {
        return raw.parse::<f64>().ok().map(|value| value / 1000.0);
    }

    if line.starts_with("out_time=") {
        let mut parts = raw.split(':');
        let hours = parts.next()?.parse::<f64>().ok()?;
        let minutes = parts.next()?.parse::<f64>().ok()?;
        let seconds = parts.next()?.parse::<f64>().ok()?;
        return Some((((hours * 60.0) + minutes) * 60.0 + seconds) * 1000.0);
    }

    None
}
