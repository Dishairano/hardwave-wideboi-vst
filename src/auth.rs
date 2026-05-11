//! Shared auth token persistence.
//!
//! All Hardwave VST plugins share the same token file at
//! `~/.local/share/hardwave/auth_token` (Linux/macOS) or the platform
//! equivalent via `dirs::data_dir()`.

use dirs;
use std::fs;
use std::path::PathBuf;

fn token_path() -> Option<PathBuf> {
    dirs::data_dir().map(|d| d.join("hardwave").join("auth_token"))
}

pub fn load_token() -> Option<String> {
    token_path()
        .and_then(|p| fs::read_to_string(p).ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

pub fn save_token(token: &str) -> Result<(), String> {
    let p = token_path().ok_or("No data dir")?;
    fs::create_dir_all(p.parent().unwrap()).map_err(|e| e.to_string())?;
    fs::write(p, token).map_err(|e| e.to_string())
}

pub fn clear_token() -> Result<(), String> {
    if let Some(p) = token_path() {
        if p.exists() {
            fs::remove_file(p).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}
