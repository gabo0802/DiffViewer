use std::path::{Path, PathBuf};
use std::fs;
use sha2::{Sha256, Digest};

/// Snapshot cache directory.
pub fn snapshot_dir() -> PathBuf {
    let base = dirs::data_dir().unwrap_or_else(|| PathBuf::from("."));
    let dir = base.join("DiffViewer").join("snapshots");
    fs::create_dir_all(&dir).ok();
    dir
}

/// Read a file's content as a string (UTF-8 with lossy fallback).
pub fn read_file_text(path: &Path) -> Result<String, String> {
    let bytes = fs::read(path).map_err(|e| format!("Failed to read {}: {}", path.display(), e))?;

    // UTF-16 LE BOM
    if bytes.len() >= 2 && bytes[0] == 0xFF && bytes[1] == 0xFE {
        let mut u16s: Vec<u16> = Vec::with_capacity((bytes.len() - 2) / 2);
        let mut i = 2usize;
        while i + 1 < bytes.len() {
            u16s.push(u16::from_le_bytes([bytes[i], bytes[i + 1]]));
            i += 2;
        }
        return Ok(String::from_utf16_lossy(&u16s));
    }

    // UTF-16 BE BOM
    if bytes.len() >= 2 && bytes[0] == 0xFE && bytes[1] == 0xFF {
        let mut u16s: Vec<u16> = Vec::with_capacity((bytes.len() - 2) / 2);
        let mut i = 2usize;
        while i + 1 < bytes.len() {
            u16s.push(u16::from_be_bytes([bytes[i], bytes[i + 1]]));
            i += 2;
        }
        return Ok(String::from_utf16_lossy(&u16s));
    }

    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

/// Create a snapshot of a file. Returns (snapshot_id, sha256, size, cache_path).
pub fn snapshot_file(path: &Path) -> Result<(String, String, i64, String), String> {
    let bytes = fs::read(path).map_err(|e| format!("Failed to read {}: {}", path.display(), e))?;
    let hash = hex_sha256(&bytes);
    let size = bytes.len() as i64;
    let snap_id = uuid::Uuid::new_v4().to_string();
    let cache_path = snapshot_dir().join(&snap_id);
    fs::write(&cache_path, &bytes).map_err(|e| format!("Failed to write snapshot: {}", e))?;
    Ok((snap_id, hash, size, cache_path.to_string_lossy().into_owned()))
}

/// Read snapshot content by cache path.
pub fn read_snapshot(cache_path: &str) -> Result<String, String> {
    read_file_text(Path::new(cache_path))
}

/// Atomic write with backup.
pub fn atomic_write(target: &Path, content: &[u8]) -> Result<(), String> {
    // Backup existing file
    if target.exists() {
        let backup = target.with_extension("diffedit.bak");
        fs::copy(target, &backup).map_err(|e| format!("Backup failed: {}", e))?;
    }
    // Write to temp file next to target
    let tmp = target.with_extension("diffedit.tmp");
    fs::write(&tmp, content).map_err(|e| format!("Write temp failed: {}", e))?;
    // Rename to target
    fs::rename(&tmp, target).map_err(|e| format!("Rename failed: {}", e))?;
    Ok(())
}

/// Detect line ending style from existing file content.
pub fn detect_eol(content: &str) -> &'static str {
    if content.contains("\r\n") { "\r\n" } else { "\n" }
}

fn hex_sha256(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    format!("{:x}", hasher.finalize())
}
