use serde::{Deserialize, Deserializer, Serialize};
use std::collections::HashMap;
use std::path::Path;

use crate::error::PluginError;

/// PM `PluginDescription` list fields (`api`, `depend`, `aliases`, ...):
/// YAML allows a single scalar (`api: 5.45.0`, as shipped by ScoreHud's
/// phar) and PHP coerces it with `(array)`/`stringList()`. Rust must accept
/// the same shapes or `serde_yaml` rejects the manifest with
/// `invalid type: string ..., expected a sequence`, which (now that the
/// PHP worker runs) counts as a load error and stops the server.
fn deserialize_string_list<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = serde_yaml::Value::deserialize(deserializer)?;
    Ok(match value {
        serde_yaml::Value::Null => Vec::new(),
        serde_yaml::Value::Sequence(items) => items
            .into_iter()
            .filter_map(|item| match item {
                serde_yaml::Value::String(text) => Some(text),
                serde_yaml::Value::Number(number) => Some(number.to_string()),
                serde_yaml::Value::Bool(flag) => Some(flag.to_string()),
                _ => None,
            })
            .collect(),
        serde_yaml::Value::String(text) if text.is_empty() => Vec::new(),
        serde_yaml::Value::String(text) => vec![text],
        serde_yaml::Value::Number(number) => vec![number.to_string()],
        serde_yaml::Value::Bool(flag) => vec![flag.to_string()],
        _ => Vec::new(),
    })
}

/// PM `stringValue()`: scalar YAML values (`version: 1.0`) coerce to text.
fn deserialize_scalar_string<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    let value = serde_yaml::Value::deserialize(deserializer)?;
    Ok(match value {
        serde_yaml::Value::String(text) => text,
        serde_yaml::Value::Number(number) => number.to_string(),
        serde_yaml::Value::Bool(flag) => flag.to_string(),
        _ => String::new(),
    })
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CommandDescription {
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub usage: Option<String>,
    #[serde(default)]
    pub permission: Option<String>,
    #[serde(default, deserialize_with = "deserialize_string_list")]
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
    #[serde(deserialize_with = "deserialize_scalar_string")]
    pub name: String,
    #[serde(deserialize_with = "deserialize_scalar_string")]
    pub version: String,
    #[serde(deserialize_with = "deserialize_scalar_string")]
    pub main: String,
    #[serde(default, deserialize_with = "deserialize_string_list")]
    pub api: Vec<String>,
    #[serde(default)]
    pub author: Option<String>,
    #[serde(default, deserialize_with = "deserialize_string_list")]
    pub authors: Vec<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub website: Option<String>,
    #[serde(default, deserialize_with = "deserialize_string_list")]
    pub depend: Vec<String>,
    #[serde(default, deserialize_with = "deserialize_string_list")]
    pub softdepend: Vec<String>,
    #[serde(
        default,
        rename = "load-before",
        alias = "loadbefore",
        alias = "loadBefore",
        deserialize_with = "deserialize_string_list"
    )]
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
    fn scalar_api_and_numeric_version_are_coerced() {
        // ScoreHud's shipped phar uses `api: 5.45.0` (scalar, not a list).
        let yml = "name: ScoreHud\nversion: 8.0.1\nmain: Ifera\\ScoreHud\\ScoreHud\napi: 5.45.0\n";
        let desc = PluginDescription::from_yaml_str(yml).expect("scalar api accepted");
        assert_eq!(desc.api, vec!["5.45.0".to_string()]);
        // PM `stringValue()` also coerces numeric scalars.
        let yml = "name: Foo\nversion: 1.0\nmain: Foo\\Main\napi: [5.0.0]\ndepend: Bar\n";
        let desc = PluginDescription::from_yaml_str(yml).expect("numeric version accepted");
        assert_eq!(desc.version, "1.0");
        assert_eq!(desc.depend, vec!["Bar".to_string()]);
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
