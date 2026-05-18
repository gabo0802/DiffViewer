use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use crate::io;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentSource {
    Virtual { text: String },
    Path { path: String },
    Snapshot { snapshot_id: String },
}

impl ContentSource {
    pub fn virtual_text(text: impl Into<String>) -> Self {
        Self::Virtual { text: text.into() }
    }

    pub fn path(path: impl Into<String>) -> Self {
        Self::Path { path: path.into() }
    }

    pub fn snapshot(snapshot_id: impl Into<String>) -> Self {
        Self::Snapshot {
            snapshot_id: snapshot_id.into(),
        }
    }

    pub fn from_json(json: &str) -> Result<Self, String> {
        serde_json::from_str(json).map_err(|err| err.to_string())
    }

    pub fn to_json_string(&self) -> String {
        serde_json::to_string(self).unwrap_or_else(|_| "{}".to_string())
    }

    pub fn resolve_text(&self) -> Result<String, String> {
        match self {
            Self::Virtual { text } => Ok(text.clone()),
            Self::Path { path } => io::read_file_text(Path::new(path)),
            Self::Snapshot { snapshot_id } => {
                let cache_path = io::snapshot_dir().join(snapshot_id);
                io::read_file_text(&cache_path)
            }
        }
    }

    pub fn resolve_json_text(json: &str) -> Result<String, String> {
        Self::from_json(json)?.resolve_text()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WriteTarget {
    Path { path: String },
    SaveAsRequired,
    ReadOnly,
}

impl WriteTarget {
    pub fn path(path: impl Into<String>) -> Self {
        Self::Path { path: path.into() }
    }

    pub fn from_json(json: &str) -> Result<Self, String> {
        serde_json::from_str(json).map_err(|err| err.to_string())
    }

    pub fn to_json_string(&self) -> String {
        serde_json::to_string(self).unwrap_or_else(|_| "{}".to_string())
    }

    pub fn resolve_save_as_target_json(
        write_target_json: &str,
        requested_path: &str,
    ) -> Result<String, String> {
        let requested = PathBuf::from(requested_path);
        if requested.is_absolute() {
            return Ok(requested.to_string_lossy().into_owned());
        }

        if let Ok(Self::Path { path }) = Self::from_json(write_target_json) {
            let existing = Path::new(&path);
            if let Some(parent) = existing.parent() {
                return Ok(parent.join(&requested).to_string_lossy().into_owned());
            }
        }

        let cwd = std::env::current_dir().map_err(|err| err.to_string())?;
        Ok(cwd.join(requested).to_string_lossy().into_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_snake_case_write_targets() {
        assert_eq!(
            WriteTarget::from_json(r#"{"type":"save_as_required"}"#).unwrap(),
            WriteTarget::SaveAsRequired
        );
        assert_eq!(
            WriteTarget::from_json(r#"{"type":"read_only"}"#).unwrap(),
            WriteTarget::ReadOnly
        );
    }

    #[test]
    fn virtual_content_resolves_without_io() {
        let source = ContentSource::from_json(r#"{"type":"virtual","text":"hello"}"#).unwrap();
        assert_eq!(source.resolve_text().unwrap(), "hello");
    }
}
