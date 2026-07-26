use crate::commands::CommandResult;

/// Bounded bridge for webview `error`/`unhandledrejection` events (see
/// `src/lib/frontendLogBridge.ts`). Each field is truncated server-side so a
/// misbehaving frontend cannot smuggle unbounded or arbitrary payloads into
/// the log stream — callers must already have sanitized to name/code/category
/// plus a short message before this boundary.
const FRONTEND_LOG_FIELD_MAX_CHARS: usize = 500;

#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize, PartialEq, Eq, specta::Type)]
#[serde(rename_all = "lowercase")]
pub enum FrontendLogLevel {
    Error,
    Warn,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct FrontendLogEntry {
    pub level: FrontendLogLevel,
    pub scope: String,
    pub message: String,
}

fn flatten_log_field(value: &str) -> String {
    value
        .chars()
        .map(|ch| match ch {
            '\n' | '\r' => ' ',
            '\t' => ' ',
            ch if ch.is_control() => '?',
            ch => ch,
        })
        .collect()
}

fn truncate_field(value: &str, max_chars: usize) -> String {
    if value.chars().count() <= max_chars {
        return value.to_string();
    }
    let truncated: String = value.chars().take(max_chars).collect();
    format!("{truncated}…")
}

/// Truncates both free-text fields of an entry to the field cap. Split out
/// from `log_frontend` so the truncation boundary is directly testable
/// without asserting against `log` macro output.
fn sanitized_fields(entry: &FrontendLogEntry) -> (String, String) {
    (
        truncate_field(
            &flatten_log_field(&entry.scope),
            FRONTEND_LOG_FIELD_MAX_CHARS,
        ),
        truncate_field(
            &flatten_log_field(&entry.message),
            FRONTEND_LOG_FIELD_MAX_CHARS,
        ),
    )
}

/// Logs a sanitized frontend failure record through the standard `log` crate
/// so webview errors and unhandled rejections show up in captured dev logs
/// (`RUST_LOG=audiobook_boss_lib=info`), which otherwise only tee Rust
/// stdout/stderr.
#[tauri::command]
#[specta::specta]
pub fn log_frontend(entry: FrontendLogEntry) -> CommandResult<()> {
    let (scope, message) = sanitized_fields(&entry);

    match entry.level {
        FrontendLogLevel::Error => {
            log::error!(target: "audiobook_boss_lib::frontend", "[{scope}] {message}")
        }
        FrontendLogLevel::Warn => {
            log::warn!(target: "audiobook_boss_lib::frontend", "[{scope}] {message}")
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncate_field_passes_short_values_through_unchanged() {
        assert_eq!(truncate_field("short message", 500), "short message");
    }

    #[test]
    fn truncate_field_bounds_long_values_to_the_char_cap() {
        let long_value = "a".repeat(600);
        let truncated = truncate_field(&long_value, 500);
        assert_eq!(truncated.chars().count(), 501); // 500 chars + the ellipsis marker
        assert!(truncated.starts_with(&"a".repeat(500)));
        assert!(truncated.ends_with('…'));
    }

    #[test]
    fn sanitized_fields_flattens_newlines_and_control_characters() {
        let entry = FrontendLogEntry {
            level: FrontendLogLevel::Error,
            scope: "window.error:Error".to_string(),
            message: "line1\nline2\r\n\tok\u{0008}bad".to_string(),
        };

        let (scope, message) = sanitized_fields(&entry);

        assert_eq!(scope, "window.error:Error");
        assert_eq!(message, "line1 line2   ok?bad");
    }

    #[test]
    fn sanitized_fields_bounds_oversized_scope_and_message() {
        let entry = FrontendLogEntry {
            level: FrontendLogLevel::Error,
            scope: "s".repeat(600),
            message: "m".repeat(600),
        };

        let (scope, message) = sanitized_fields(&entry);

        assert_eq!(scope.chars().count(), 501);
        assert_eq!(message.chars().count(), 501);
        assert!(scope.ends_with('…'));
        assert!(message.ends_with('…'));
    }

    #[test]
    fn log_frontend_accepts_oversized_input_without_erroring() {
        let result = log_frontend(FrontendLogEntry {
            level: FrontendLogLevel::Warn,
            scope: "s".repeat(600),
            message: "m".repeat(600),
        });
        assert!(result.is_ok());
    }
}
