use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::io;

static DEBUG_ENABLED: AtomicBool = AtomicBool::new(false);

pub fn configure_from_args(args: &[String]) {
    let enabled = args.iter().any(|arg| arg == "--debug");
    DEBUG_ENABLED.store(enabled, Ordering::Relaxed);
}

pub fn is_enabled() -> bool {
    DEBUG_ENABLED.load(Ordering::Relaxed)
}

pub struct DebugLogger {
    scope: &'static str,
}

impl DebugLogger {
    pub fn new(scope: &'static str) -> Self {
        Self { scope }
    }

    pub fn log(&self, message: impl AsRef<str>) {
        if !is_enabled() {
            return;
        }
        eprintln!("[diffviewer-debug][{}] {}", self.scope, message.as_ref());
    }

    pub fn log_multiline(&self, label: &str, text: &str) {
        if !is_enabled() || text.trim().is_empty() {
            return;
        }
        eprintln!("[diffviewer-debug][{}] {}:\n{}", self.scope, label, text);
    }

    pub fn log_diff_line_counts(
        &self,
        display_path: &str,
        prev_content_json: &str,
        new_content_json: &str,
    ) {
        if !is_enabled() {
            return;
        }

        let prev_count = describe_content_source_line_count(prev_content_json);
        let new_count = describe_content_source_line_count(new_content_json);
        eprintln!(
            "[diffviewer-debug][{}] diff_line_counts path={} prev={} new={}",
            self.scope, display_path, prev_count, new_count
        );
    }

    pub fn log_merge_line_count(&self, display_path: &str, merged_text: &str, context: &str) {
        if !is_enabled() {
            return;
        }

        eprintln!(
            "[diffviewer-debug][{}] merge_line_count path={} context={} merge={}",
            self.scope,
            display_path,
            context,
            count_lines(merged_text)
        );
    }
}

fn describe_content_source_line_count(content_json: &str) -> String {
    match content_source_line_count(content_json) {
        Ok(count) => count.to_string(),
        Err(err) => format!("error({})", err),
    }
}

fn content_source_line_count(content_json: &str) -> Result<usize, String> {
    let value: serde_json::Value = serde_json::from_str(content_json).map_err(|e| e.to_string())?;
    match value.get("type").and_then(|entry| entry.as_str()) {
        Some("virtual") => Ok(count_lines(
            value
                .get("text")
                .and_then(|entry| entry.as_str())
                .unwrap_or(""),
        )),
        Some("path") => {
            let path = value
                .get("path")
                .and_then(|entry| entry.as_str())
                .ok_or("Missing path")?;
            let text = io::read_file_text(Path::new(path))?;
            Ok(count_lines(&text))
        }
        Some("snapshot") => {
            let snapshot_id = value
                .get("snapshot_id")
                .and_then(|entry| entry.as_str())
                .ok_or("Missing snapshot_id")?;
            let text = io::read_file_text(&io::snapshot_dir().join(snapshot_id))?;
            Ok(count_lines(&text))
        }
        Some(other) => Err(format!("Unsupported content source type {}", other)),
        None => Err("Missing content source type".to_string()),
    }
}

fn count_lines(text: &str) -> usize {
    text.split('\n').count()
}

#[cfg(test)]
fn debug_enabled_from<F>(args: &[String], env_lookup: F) -> bool
where
    F: Fn(&str) -> Option<String>,
{
    args.iter().any(|arg| arg == "--debug") || env_flag_enabled(&env_lookup, "npm_config_debug")
}

#[cfg(test)]
fn env_flag_enabled<F>(env_lookup: F, key: &str) -> bool
where
    F: Fn(&str) -> Option<String>,
{
    env_lookup(key)
        .map(|value| value.trim().to_ascii_lowercase())
        .map(|value| !value.is_empty() && value != "0" && value != "false" && value != "no")
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_empty_text_as_single_editor_line() {
        assert_eq!(count_lines(""), 1);
    }

    #[test]
    fn counts_trailing_newline_as_extra_line() {
        assert_eq!(count_lines("one\ntwo\n"), 3);
    }

    #[test]
    fn enables_debug_from_args() {
        let args = vec!["diffviewer.exe".to_string(), "--debug".to_string()];
        assert!(debug_enabled_from(&args, |_| None));
    }

    #[test]
    fn enables_debug_from_npm_config_env() {
        let args = vec!["diffviewer.exe".to_string()];
        assert!(debug_enabled_from(&args, |key| {
            if key == "npm_config_debug" {
                Some("true".to_string())
            } else {
                None
            }
        }));
    }

    #[test]
    fn ignores_falsey_npm_config_env() {
        let args = vec!["diffviewer.exe".to_string()];
        assert!(!debug_enabled_from(&args, |key| {
            if key == "npm_config_debug" {
                Some("false".to_string())
            } else {
                None
            }
        }));
    }
}
