use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

use crate::ui;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemanticType {
    pub name: String,
    pub description: String,
}

const DEFAULT_CONFIG_STR: &str = include_str!("default_config.toml");

#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
    #[serde(default = "default_version")]
    pub version: u32,
    pub api_key: Option<String>,
    pub model: Option<String>,
    pub semantic_types: Option<Vec<SemanticType>>,
}

fn default_config() -> Config {
    toml::from_str(DEFAULT_CONFIG_STR).expect("default_config.toml is invalid")
}

fn default_version() -> u32 {
    default_config().version
}

pub fn get_semantic_types() -> Result<Vec<SemanticType>, String> {
    let mut config = load_config()?;
    match config.semantic_types {
        Some(ref types) => Ok(types.clone()),
        None => {
            let defaults = default_config();
            let types = defaults.semantic_types.unwrap();
            config.semantic_types = Some(types.clone());
            save_config(&config)?;
            Ok(types)
        }
    }
}

/// Return the path to the config file: ~/.config/quick-commit/config.toml
fn config_path() -> Result<PathBuf, String> {
    let home = dirs::home_dir().ok_or_else(|| "Could not determine home directory".to_string())?;
    Ok(home
        .join(".config")
        .join("quick-commit")
        .join("config.toml"))
}

/// Load config from disk, returning a default Config if the file doesn't exist.
fn load_config() -> Result<Config, String> {
    let path = config_path()?;
    if !path.exists() {
        return Ok(Config {
            version: default_config().version,
            api_key: None,
            model: None,
            semantic_types: None,
        });
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

/// Resolve the API key from config file, prompting the user if not set.
pub fn get_api_key() -> Result<String, String> {
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

/// Resolve the model from config file, prompting the user if not set.
pub fn get_model() -> Result<String, String> {
    let mut config = load_config()?;
    if let Some(ref model) = config.model {
        if !model.is_empty() {
            return Ok(model.clone());
        }
    }

    // 3. Prompt the user (default if empty)
    let default_model = default_config()
        .model
        .unwrap_or_else(|| "openai/gpt-oss-20b".to_string());
    let prompt = format!("Enter OpenRouter model [{}]: ", default_model);
    let input = ui::prompt_input(&prompt);
    let model = if input.is_empty() {
        default_model
    } else {
        input
    };

    // Save for next time
    config.model = Some(model.clone());
    save_config(&config)?;

    let path = config_path().unwrap_or_default();
    println!("Model saved to {}", path.display());

    Ok(model)
}
