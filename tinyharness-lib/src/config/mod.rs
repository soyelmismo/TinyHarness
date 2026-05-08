use std::{fmt, str::FromStr};

use serde::{Deserialize, Serialize};

use crate::mode::AgentMode;

/// Identifies which provider backend was used last.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum ProviderKind {
    Ollama,
    LlamaCpp,
    Vllm,
}

impl fmt::Display for ProviderKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ProviderKind::Ollama => f.write_str("ollama"),
            ProviderKind::LlamaCpp => f.write_str("llama.cpp"),
            ProviderKind::Vllm => f.write_str("vllm"),
        }
    }
}

impl FromStr for ProviderKind {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_lowercase().as_str() {
            "ollama" => Ok(ProviderKind::Ollama),
            "llama.cpp" | "llamacpp" | "llama_cpp" => Ok(ProviderKind::LlamaCpp),
            "vllm" => Ok(ProviderKind::Vllm),
            _ => Err(format!("Unknown provider '{}'", s)),
        }
    }
}

/// Persisted application settings — pure data type with no I/O.
///
/// Use `SettingsStore` to load and save instances of this type.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub last_provider: ProviderKind,
    pub last_model: Option<String>,
    pub preferred_mode: AgentMode,
    pub ollama_api_key: Option<String>,
    /// Timeout in seconds for Ollama requests (default: 5)
    pub ollama_timeout_secs: u64,
    /// Maximum number of retries for Ollama requests (default: 3)
    pub ollama_max_retries: u32,
    /// Context limit for warning calculations only (default: None, uses model default)
    pub context_limit: Option<u32>,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            last_provider: ProviderKind::Ollama,
            last_model: None,
            preferred_mode: AgentMode::Casual,
            ollama_api_key: None,
            ollama_timeout_secs: 5,
            ollama_max_retries: 3,
            context_limit: None,
        }
    }
}

// ── Errors ──────────────────────────────────────────────────────────────────

/// Error type for settings I/O operations.
#[derive(Debug)]
pub enum SettingsError {
    Io(std::io::Error),
    Parse(serde_json::Error),
}

impl std::fmt::Display for SettingsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SettingsError::Io(e) => write!(f, "I/O error: {}", e),
            SettingsError::Parse(e) => write!(f, "parse error: {}", e),
        }
    }
}

impl std::error::Error for SettingsError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            SettingsError::Io(e) => Some(e),
            SettingsError::Parse(e) => Some(e),
        }
    }
}

impl From<std::io::Error> for SettingsError {
    fn from(e: std::io::Error) -> Self {
        SettingsError::Io(e)
    }
}

impl From<serde_json::Error> for SettingsError {
    fn from(e: serde_json::Error) -> Self {
        SettingsError::Parse(e)
    }
}

// ── Settings store ─────────────────────────────────────────────────────────

/// Handles persistence of [`Settings`] to disk.
///
/// By default, uses `~/.config/tinyharness/settings.json`. This can be
/// overridden with [`SettingsStore::new`] for testing or alternative paths.
pub struct SettingsStore {
    path: std::path::PathBuf,
}

impl SettingsStore {
    /// Create a store that reads/writes from the default path
    /// (`~/.config/tinyharness/settings.json`).
    pub fn default_path() -> Self {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        let path = std::path::PathBuf::from(home).join(".config/tinyharness/settings.json");
        SettingsStore { path }
    }

    /// Create a store that reads/writes from a custom path.
    pub fn new(path: std::path::PathBuf) -> Self {
        SettingsStore { path }
    }

    /// Load settings from disk.
    ///
    /// Returns `Ok(Settings)` with defaults if the file doesn't exist,
    /// or an error if the file exists but cannot be parsed.
    pub fn load(&self) -> Result<Settings, SettingsError> {
        if !self.path.exists() {
            return Ok(Settings::default());
        }
        let content = std::fs::read_to_string(&self.path)?;
        let settings = serde_json::from_str(&content)?;
        Ok(settings)
    }

    /// Load settings from disk, returning defaults on any error.
    ///
    /// This matches the original behaviour and is suitable for application
    /// startup where you don't want to fail on corrupt settings files.
    pub fn load_or_default(&self) -> Settings {
        self.load().unwrap_or_else(|e| {
            eprintln!("Warning: Failed to load settings: {}. Using defaults.", e);
            Settings::default()
        })
    }

    /// Save settings to disk atomically (write to temp file, then rename).
    pub fn save(&self, settings: &Settings) -> Result<(), SettingsError> {
        let dir = self.path.parent().ok_or_else(|| {
            SettingsError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "settings path has no parent directory",
            ))
        })?;
        std::fs::create_dir_all(dir)?;

        let json = serde_json::to_string_pretty(settings)?;
        let tmp_path = dir.join("settings.json.tmp");

        {
            let mut file = std::fs::File::create(&tmp_path)?;
            std::io::Write::write_all(&mut file, json.as_bytes())?;
            std::io::Write::flush(&mut file)?;
        }

        std::fs::rename(&tmp_path, &self.path)?;
        Ok(())
    }

    /// Returns the path where settings are stored.
    pub fn path(&self) -> &std::path::Path {
        &self.path
    }
}

// ── Backward-compatible convenience methods on Settings ────────────────────

impl Settings {
    /// Load settings from the default path, returning defaults on any error.
    ///
    /// Convenience method that delegates to [`SettingsStore::default_path().load_or_default()`].
    pub fn load() -> Self {
        load_settings()
    }

    /// Save settings to the default path atomically.
    ///
    /// On error, prints a warning to stderr.
    /// Convenience method that delegates to [`save_settings`].
    pub fn save(&self) {
        save_settings(self);
    }
}

// ── Free convenience functions ─────────────────────────────────────────────

/// Load settings from the default path, returning defaults on any error.
///
/// This is a convenience wrapper around [`SettingsStore::default_path().load_or_default()`].
pub fn load_settings() -> Settings {
    SettingsStore::default_path().load_or_default()
}

/// Save settings to the default path atomically.
///
/// This is a convenience wrapper around [`SettingsStore::default_path().save()`].
/// On error, prints a warning to stderr (matching the original behaviour).
pub fn save_settings(settings: &Settings) {
    let store = SettingsStore::default_path();
    if let Err(e) = store.save(settings) {
        eprintln!("Warning: Failed to save settings: {}", e);
    }
}
