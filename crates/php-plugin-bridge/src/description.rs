use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

use crate::error::PluginError;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CommandDescription {
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub usage: Option<String>,
    #[serde(default)]
    pub permission: Option<String>,
    #[serde(default)]
    pub aliases: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PermissionDescription {
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default = "default_permission_default")]
    pub default: String, // "op", "true", "false", "notop"
}

fn default_permission_default() -> String {
    "op".to_string()
}

/// Representation of a PocketMine-MP `plugin.yml` file.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PluginDescription {
    pub name: String,
    pub version: String,
    pub main: String,
    #[serde(default)]
    pub api: Vec<String>,
    #[serde(default)]
    pub author: Option<String>,
    #[serde(default)]
    pub authors: Vec<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub website: Option<String>,
    #[serde(default)]
    pub depend: Vec<String>,
    #[serde(default)]
    pub softdepend: Vec<String>,
    #[serde(default)]
    pub load_before: Vec<String>,
    #[serde(default)]
    pub commands: HashMap<String, CommandDescription>,
    #[serde(default)]
    pub permissions: HashMap<String, PermissionDescription>,
}

impl PluginDescription {
    pub fn from_yaml_str(content: &str) -> Result<Self, PluginError> {
        let desc: PluginDescription = serde_yaml::from_str(content)?;
        desc.validate()?;
        Ok(desc)
    }

    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self, PluginError> {
        let content = std::fs::read_to_string(path)?;
        Self::from_yaml_str(&content)
    }

    pub fn validate(&self) -> Result<(), PluginError> {
        if self.name.trim().is_empty() {
            return Err(PluginError::InvalidManifest("unknown".to_string(), "name"));
        }
        if self.version.trim().is_empty() {
            return Err(PluginError::InvalidManifest(self.name.clone(), "version"));
        }
        if self.main.trim().is_empty() {
            return Err(PluginError::InvalidManifest(self.name.clone(), "main"));
        }
        Ok(())
    }

    pub fn all_authors(&self) -> Vec<String> {
        let mut authors = self.authors.clone();
        if let Some(author) = &self.author {
            if !authors.contains(author) {
                authors.push(author.clone());
            }
        }
        authors
    }
}
