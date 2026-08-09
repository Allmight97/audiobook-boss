use crate::commands::CommandResult;

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
            '\n' | '\r' | '\t' => ' ',
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
    fn sanitized_fields_flattens_and_bounds_input() {
        let entry = FrontendLogEntry {
            level: FrontendLogLevel::Error,
            scope: "s".repeat(600),
            message: format!("line1\nline2\t{}", "m".repeat(600)),
        };

        let (scope, message) = sanitized_fields(&entry);
        assert_eq!(scope.chars().count(), 501);
        assert_eq!(message.chars().count(), 501);
        assert!(!message.contains('\n'));
        assert!(!message.contains('\t'));
    }

    #[test]
    fn log_frontend_accepts_oversized_input() {
        assert!(log_frontend(FrontendLogEntry {
            level: FrontendLogLevel::Warn,
            scope: "s".repeat(600),
            message: "m".repeat(600),
        })
        .is_ok());
    }
}
