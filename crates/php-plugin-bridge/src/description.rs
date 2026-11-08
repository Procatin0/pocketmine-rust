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
    #[serde(default, rename = "load-before", alias = "loadbefore", alias = "loadBefore")]
    pub load_before: Vec<String>,
    /// `src-namespace-prefix` in `plugin.yml` (PM `PluginDescription::
    /// getSrcNamespacePrefix()`). Empty means "not set": the PHP autoloader
    /// then falls back to the namespace of `main` (see `effective_src_prefix`).
    #[serde(default, rename = "src-namespace-prefix")]
    pub src_namespace_prefix: String,
    /// `load: startup|postworld` (`PluginEnableOrder`); `None` means the PM
    /// default, `postworld`.
    #[serde(default)]
    pub load: Option<String>,
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
        if let Some(load) = &self.load {
            let normalized = load.trim().to_ascii_lowercase();
            if normalized != "startup" && normalized != "postworld" {
                return Err(PluginError::InvalidManifest(
                    self.name.clone(),
                    "load",
                ));
            }
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

    /// `PluginDescription::getFullName()`: `"{name} v{version}"`.
    /// Used by `Loading`/`Enabling`/`Disabling` log lines.
    pub fn full_name(&self) -> String {
        format!("{} v{}", self.name, self.version)
    }

    /// Whether this plugin enables in the `STARTUP` phase (`load: startup`).
    pub fn is_startup(&self) -> bool {
        matches!(
            self.load.as_deref().map(str::trim).map(str::to_ascii_lowercase),
            Some(ref v) if v == "startup"
        )
    }

    /// Namespace of the `main` class, e.g. `Acme\Foo` for `Acme\Foo\Main`,
    /// or an empty string when `main` has no namespace.
    pub fn main_namespace(&self) -> String {
        let main = self.main.trim().trim_start_matches('\\');
        match main.rfind('\\') {
            Some(index) => main[..index].to_string(),
            None => String::new(),
        }
    }

    /// Effective PSR-4 prefix for the PHP autoloader: the manifest's
    /// `src-namespace-prefix` when set, otherwise the namespace of `main`.
    ///
    /// The result is normalized to `Foo\Bar\` form (trailing backslash) so it
    /// can be compared directly against FQCNs; it is empty only when `main`
    /// has no namespace and no prefix is configured.
    pub fn effective_src_prefix(&self) -> String {
        let explicit = self.src_namespace_prefix.trim().trim_matches(|c| c == '\\' || c == '/');
        let base = if !explicit.is_empty() {
            explicit.to_string()
        } else {
            self.main_namespace()
        };
        let base = base.replace('/', "\\");
        if base.is_empty() {
            String::new()
        } else {
            format!("{base}\\")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_src_namespace_prefix_and_load_before() {
        let yml = "name: BetterChat\nversion: 1.0.0\nmain: BetterChat\\Main\nsrc-namespace-prefix: BetterChat\napi: [5.0.0]\nload-before: [ChatGuard]\n";
        let desc = PluginDescription::from_yaml_str(yml).expect("valid manifest");
        assert_eq!(desc.src_namespace_prefix, "BetterChat");
        assert_eq!(desc.load_before, vec!["ChatGuard".to_string()]);
        assert_eq!(desc.effective_src_prefix(), "BetterChat\\");
    }

    #[test]
    fn effective_src_prefix_defaults_to_main_namespace() {
        let yml = "name: ExamplePlugin\nversion: 1.1.0\nmain: pocketmine\\example\\Main\napi: [5.0.0]\n";
        let desc = PluginDescription::from_yaml_str(yml).expect("valid manifest");
        assert_eq!(desc.src_namespace_prefix, "");
        assert_eq!(desc.main_namespace(), "pocketmine\\example");
        assert_eq!(desc.effective_src_prefix(), "pocketmine\\example\\");
    }

    #[test]
    fn explicit_prefix_wins_over_main_namespace() {
        let yml = "name: Nested\nversion: 1.0.0\nmain: Acme\\Foo\\Sub\\Main\nsrc-namespace-prefix: Acme\\Foo\napi: [5.0.0]\n";
        let desc = PluginDescription::from_yaml_str(yml).expect("valid manifest");
        assert_eq!(desc.main_namespace(), "Acme\\Foo\\Sub");
        assert_eq!(desc.effective_src_prefix(), "Acme\\Foo\\");
    }
}
