use std::path::PathBuf;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum PluginError {
    #[error("Plugin manifest (plugin.yml) missing in {0}")]
    MissingManifest(PathBuf),

    #[error("Failed to parse plugin.yml: {0}")]
    ManifestParseError(#[from] serde_yaml::Error),

    #[error("Plugin '{0}' is missing mandatory field: {1}")]
    InvalidManifest(String, &'static str),

    #[error("Circular dependency detected involving plugin '{0}'")]
    CircularDependency(String),

    #[error("Missing dependency '{0}' required by plugin '{1}'")]
    MissingDependency(String, String),

    #[error("Plugin '{0}' is already registered")]
    AlreadyRegistered(String),

    #[error("Plugin '{0}' not found")]
    NotFound(String),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("PHP runtime error: {0}")]
    PhpRuntime(String),

    #[error("Serialization error: {0}")]
    Json(#[from] serde_json::Error),
}
