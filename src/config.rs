use serde::{Deserialize, Serialize};
use std::env;
use std::fs;
use std::path::PathBuf;

use crate::ui;

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Config {
    pub api_key: Option<String>,
}

/// Return the path to the config file: ~/.config/quick-commit/config.toml
fn config_path() -> Result<PathBuf, String> {
    let home = dirs::home_dir()
        .ok_or_else(|| "Could not determine home directory".to_string())?;
    Ok(home.join(".config").join("quick-commit").join("config.toml"))
}

/// Load config from disk, returning a default Config if the file doesn't exist.
fn load_config() -> Result<Config, String> {
    let path = config_path()?;
    if !path.exists() {
        return Ok(Config::default());
    }
    let contents =
        fs::read_to_string(&path).map_err(|e| format!("Failed to read config: {}", e))?;
    toml::from_str(&contents).map_err(|e| format!("Failed to parse config: {}", e))
}

/// Save config to disk, creating parent directories if needed.
fn save_config(config: &Config) -> Result<(), String> {
    let path = config_path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create config directory: {}", e))?;
    }
    let contents =
        toml::to_string_pretty(config).map_err(|e| format!("Failed to serialize config: {}", e))?;
    fs::write(&path, contents).map_err(|e| format!("Failed to write config: {}", e))
}

/// Resolve the API key with the following priority:
/// 1. OPENROUTER_API_KEY env var
/// 2. Config file (~/.config/quick-commit/config.toml)
/// 3. Prompt the user, then save to config file
pub fn get_api_key() -> Result<String, String> {
    // 1. Check env var first (allows overrides / CI usage)
    if let Ok(key) = env::var("OPENROUTER_API_KEY") {
        if !key.is_empty() {
            return Ok(key);
        }
    }

    // 2. Check config file
    let mut config = load_config()?;
    if let Some(ref key) = config.api_key {
        if !key.is_empty() {
            return Ok(key.clone());
        }
    }

    // 3. Prompt the user
    let key = ui::prompt_input("Enter your OpenRouter API key: ");
    if key.is_empty() {
        return Err("No API key provided".to_string());
    }

    // Save for next time
    config.api_key = Some(key.clone());
    save_config(&config)?;

    let path = config_path().unwrap_or_default();
    println!("API key saved to {}", path.display());

    Ok(key)
}
