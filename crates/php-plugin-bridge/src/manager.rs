use parking_lot::RwLock;
use serde_json::json;
use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::sync::Mutex as TokioMutex;
use tracing::{error, info, warn};

use crate::command::{CommandMap, CommandSender};
use crate::context::PluginAction;
use crate::description::PluginDescription;
use crate::error::PluginError;
use crate::event::{Event, EventDispatcher};
use crate::host::PluginHost;
use crate::plugin::Plugin;
use crate::runtime::{DiscoveredPluginManifest, PhpPlugin, PhpWorker, PluginSource};

/// Result of executing a server command.
#[derive(Debug, Clone, PartialEq)]
pub struct CommandOutcome {
    pub handled: bool,
    pub actions: Vec<PluginAction>,
}

pub struct PluginManager {
    plugins: RwLock<HashMap<String, Arc<TokioMutex<dyn Plugin>>>>,
    descriptions: RwLock<HashMap<String, PluginDescription>>,
    dispatcher: Arc<EventDispatcher>,
    command_map: Arc<CommandMap>,
    php_worker: Arc<PhpWorker>,
    host: RwLock<Option<Arc<dyn PluginHost>>>,
    load_order: RwLock<Vec<String>>,
    /// listener events per plugin, used to rebuild the union set.
    listener_events_by_plugin: RwLock<HashMap<String, Vec<String>>>,
    listener_events: RwLock<HashSet<String>>,
    current_tick: AtomicU64,
}

impl Default for PluginManager {
    fn default() -> Self {
        Self::new()
    }
}

impl PluginManager {
    pub fn new() -> Self {
        Self {
            plugins: RwLock::new(HashMap::new()),
            descriptions: RwLock::new(HashMap::new()),
            dispatcher: Arc::new(EventDispatcher::new()),
            command_map: Arc::new(CommandMap::new()),
            php_worker: Arc::new(PhpWorker::new()),
            host: RwLock::new(None),
            load_order: RwLock::new(Vec::new()),
            listener_events_by_plugin: RwLock::new(HashMap::new()),
            listener_events: RwLock::new(HashSet::new()),
            current_tick: AtomicU64::new(0),
        }
    }

    pub fn dispatcher(&self) -> &Arc<EventDispatcher> {
        &self.dispatcher
    }

    pub fn command_map(&self) -> &Arc<CommandMap> {
        &self.command_map
    }

    pub fn php_worker(&self) -> &Arc<PhpWorker> {
        &self.php_worker
    }

    /// Installs the host used for synchronous PHP -> Rust calls.
    ///
    /// The host is shared with the worker, so it also applies if the runtime
    /// is started later.
    pub fn set_host(&self, host: Arc<dyn PluginHost>) {
        *self.host.write() = Some(Arc::clone(&host));
        self.php_worker.set_host(host);
    }

    pub fn plugin_count(&self) -> usize {
        self.plugins.read().len()
    }

    pub fn get_description(&self, name: &str) -> Option<PluginDescription> {
        self.descriptions.read().get(name).cloned()
    }

    /// Planned load order (PHP plugins, dependencies first).
    pub fn load_order(&self) -> Vec<String> {
        self.load_order.read().clone()
    }

    pub fn current_tick(&self) -> u64 {
        self.current_tick.load(Ordering::SeqCst)
    }

    /// Initializes the PHP background worker.
    pub async fn init_runtime(&self) -> Result<(), PluginError> {
        match self.php_worker.start().await {
            Ok(true) => {}
            Ok(false) => {
                warn!("PHP runtime unavailable; PHP plugins will not be enabled");
            }
            Err(error) => {
                warn!("Failed to initialize PHP runtime: {}", error);
            }
        }
        Ok(())
    }

    /// Registers a native Rust plugin manually.
    pub async fn register_native_plugin<P: Plugin + 'static>(
        &self,
        plugin: P,
    ) -> Result<(), PluginError> {
        let name = plugin.name().to_string();
        let desc = plugin.description().clone();

        {
            let mut map = self.plugins.write();
            if map.contains_key(&name) {
                return Err(PluginError::AlreadyRegistered(name));
            }
            map.insert(name.clone(), Arc::new(TokioMutex::new(plugin)));
        }
        self.descriptions.write().insert(name, desc);
        Ok(())
    }

    /// Scans a directory (e.g. `plugins/`) and loads all plugins respecting
    /// dependencies.
    ///
    /// When PHP is available discovery (including `.phar` archives) is
    /// delegated to the runtime. Otherwise a native directory scan is used.
    pub async fn load_plugins_from_directory<P: AsRef<Path>>(
        &self,
        dir: P,
    ) -> Result<usize, PluginError> {
        let path = dir.as_ref();
        if !path.exists() {
            std::fs::create_dir_all(path)?;
            return Ok(0);
        }

        let mut discovered: Vec<(PluginDescription, PluginSource)> = Vec::new();

        for item in self.discover_plugins_with_php(path) {
            match Self::parse_discovered_manifest(&item) {
                Ok(plugin) => discovered.push(plugin),
                Err(error) => warn!(
                    "Failed to parse discovered plugin manifest at {}: {}",
                    item.path, error
                ),
            }
        }

        if discovered.is_empty() {
            if !self.php_worker.is_available() {
                warn!(
                    "PHP runtime unavailable; using native directory plugin discovery in {}",
                    path.display()
                );
            }
            discovered = Self::discover_plugin_directories(path);
        }

        if discovered.is_empty() {
            info!("No plugins found in {}", path.display());
            *self.load_order.write() = Vec::new();
            return Ok(0);
        }

        let ordered = Self::resolve_dependencies_with_sources(&discovered)?;
        let order: Vec<String> = ordered
            .iter()
            .map(|(description, _)| description.name.clone())
            .collect();
        info!(
            "Calculated load order for {} plugins: {:?}",
            order.len(),
            order
        );
        *self.load_order.write() = order;

        let mut loaded_count = 0;
        for (description, source) in ordered {
            let name = description.name.clone();

            if self.plugins.read().contains_key(&name) {
                warn!("Plugin '{}' is already loaded; skipping", name);
                continue;
            }

            let plugin: Arc<TokioMutex<dyn Plugin>> = Arc::new(TokioMutex::new(PhpPlugin::new(
                description.clone(),
                source,
                Arc::clone(&self.php_worker),
            )));

            {
                let mut guard = plugin.lock().await;
                if let Err(error) = guard.on_load().await {
                    error!("Error during onLoad for plugin '{}': {}", name, error);
                    continue;
                }
            }

            self.plugins.write().insert(name.clone(), plugin);
            self.descriptions.write().insert(name, description);
            loaded_count += 1;
        }

        Ok(loaded_count)
    }

    /// Enables all loaded plugins in dependency order.
    pub async fn enable_plugins(&self) -> Result<(), PluginError> {
        for name in self.ordered_plugin_names() {
            let plugin = { self.plugins.read().get(&name).cloned() };
            let Some(plugin) = plugin else {
                continue;
            };

            let mut guard = plugin.lock().await;
            if guard.is_enabled() {
                continue;
            }

            match guard.on_enable().await {
                Ok(()) => {
                    let commands = guard.commands();
                    let events = guard.listener_events();
                    drop(guard);

                    if !commands.is_empty() {
                        info!(
                            "Plugin '{}' registered {} command(s): {:?}",
                            name,
                            commands.len(),
                            commands
                        );
                    }

                    self.record_plugin_listeners(&name, events);
                }
                Err(error) => {
                    error!("Error enabling plugin '{}': {}", name, error);
                }
            }
        }

        self.refresh_listener_events();
        Ok(())
    }

    /// Disables all loaded plugins in reverse dependency order.
    pub async fn disable_plugins(&self) -> Result<(), PluginError> {
        let mut names = self.ordered_plugin_names();
        names.reverse();

        for name in names {
            let plugin = { self.plugins.read().get(&name).cloned() };
            let Some(plugin) = plugin else {
                continue;
            };

            {
                let mut guard = plugin.lock().await;
                if let Err(error) = guard.on_disable().await {
                    error!("Error disabling plugin '{}': {}", name, error);
                }
            }

            self.record_plugin_listeners(&name, Vec::new());
        }

        self.refresh_listener_events();
        Ok(())
    }

    /// Dispatches an event through native listeners and then PHP.
    ///
    /// The final PHP event snapshot is hydrated back into `event`, including
    /// cancellation state, and all emitted actions are merged.
    pub fn dispatch_event_sync(
        &self,
        event: &mut dyn Event,
    ) -> Result<Vec<PluginAction>, PluginError> {
        let mut actions = self.dispatcher.dispatch(event)?;

        if self.php_worker.is_available() {
            match self.php_worker.request_sync(
                "dispatch_event",
                json!({
                    "event": event.name(),
                    "data": event.to_json()
                }),
            ) {
                Ok(response) => {
                    if response.status != "ok" {
                        warn!(
                            "PHP dispatch of '{}' returned status '{}': {:?}",
                            event.name(),
                            response.status,
                            response.message
                        );
                    } else {
                        if let Some(value) = &response.event {
                            event.apply_json(value);
                        }
                        if response.cancelled && event.is_cancellable() {
                            event.set_cancelled(true);
                        }
                        actions.extend(response.actions);
                    }
                }
                Err(error) => {
                    // A broken PHP worker must not break event handling.
                    warn!("PHP dispatch_event failed: {}", error);
                }
            }
        }

        Ok(actions)
    }

    /// Async wrapper around [`PluginManager::dispatch_event_sync`].
    pub async fn dispatch_event(
        &self,
        event: &mut dyn Event,
    ) -> Result<Vec<PluginAction>, PluginError> {
        self.dispatch_event_sync(event)
    }

    /// Executes a command (legacy API returning only the emitted actions).
    ///
    /// Delegates to [`PluginManager::execute_command_sync`], so native
    /// commands keep priority and PHP commands are used as a fallback.
    pub fn execute_command(
        &self,
        sender: &CommandSender,
        command_line: &str,
    ) -> Result<Option<Vec<PluginAction>>, PluginError> {
        Ok(self
            .execute_command_sync(sender, command_line)?
            .map(|outcome| outcome.actions))
    }

    /// Executes a command natively first, then through PHP plugin commands.
    pub fn execute_command_sync(
        &self,
        sender: &CommandSender,
        command_line: &str,
    ) -> Result<Option<CommandOutcome>, PluginError> {
        if let Some(actions) = self.command_map.execute(sender, command_line)? {
            return Ok(Some(CommandOutcome {
                handled: true,
                actions,
            }));
        }

        if !self.php_worker.is_available() {
            return Ok(None);
        }

        let Some((command, args)) = split_command_line(command_line) else {
            return Ok(None);
        };

        let response = self.php_worker.request_sync(
            "execute_command",
            json!({
                "sender": sender_to_json(sender),
                "command": command,
                "args": args
            }),
        )?;

        if response.status != "ok" {
            warn!(
                "PHP command '{}' failed with status '{}': {:?}",
                command, response.status, response.message
            );
            return Ok(None);
        }

        if response.handled {
            Ok(Some(CommandOutcome {
                handled: true,
                actions: response.actions,
            }))
        } else {
            Ok(None)
        }
    }

    /// Runs one server tick inside the PHP scheduler.
    pub fn tick_sync(&self, tick: u64) -> Result<Vec<PluginAction>, PluginError> {
        self.current_tick.store(tick, Ordering::SeqCst);

        if !self.php_worker.is_available() {
            return Ok(Vec::new());
        }

        let response = self
            .php_worker
            .request_sync("tick", json!({ "tick": tick }))?;

        if response.status != "ok" {
            warn!(
                "PHP tick {} returned status '{}': {:?}",
                tick, response.status, response.message
            );
            return Ok(Vec::new());
        }

        Ok(response.actions)
    }

    /// Whether PHP has any listener for `event_name`.
    pub fn has_listeners(&self, event_name: &str) -> bool {
        self.listener_events.read().contains(event_name)
    }

    /// Rebuilds the listener-name union from the per-plugin metadata.
    pub fn refresh_listener_events(&self) {
        let by_plugin = self.listener_events_by_plugin.read();
        let mut combined = HashSet::new();
        for events in by_plugin.values() {
            combined.extend(events.iter().cloned());
        }
        *self.listener_events.write() = combined;
    }

    fn record_plugin_listeners(&self, plugin_name: &str, events: Vec<String>) {
        self.listener_events_by_plugin
            .write()
            .insert(plugin_name.to_string(), events);
        self.refresh_listener_events();
    }

    /// Names in load order followed by native/extra plugins (alphabetical).
    fn ordered_plugin_names(&self) -> Vec<String> {
        let plugins = self.plugins.read();
        let order = self.load_order.read();

        let mut names = Vec::new();
        for name in order.iter() {
            if plugins.contains_key(name) && !names.contains(name) {
                names.push(name.clone());
            }
        }

        let mut remaining: Vec<String> = plugins
            .keys()
            .filter(|name| !names.contains(*name))
            .cloned()
            .collect();
        remaining.sort();
        names.extend(remaining);
        names
    }

    fn discover_plugins_with_php(&self, path: &Path) -> Vec<DiscoveredPluginManifest> {
        if !self.php_worker.is_available() {
            return Vec::new();
        }

        match self
            .php_worker
            .request_sync("discover_plugins", json!({ "dir": path.to_string_lossy() }))
        {
            Ok(response) if response.status == "ok" => response.plugins,
            Ok(response) => {
                warn!(
                    "PHP plugin discovery returned status '{}': {:?}",
                    response.status, response.message
                );
                Vec::new()
            }
            Err(error) => {
                warn!("PHP plugin discovery failed: {}", error);
                Vec::new()
            }
        }
    }

    /// Native fallback used when PHP is unavailable: directories with a
    /// `plugin.yml`. Phar archives require PHP's `phar://` support.
    fn discover_plugin_directories(path: &Path) -> Vec<(PluginDescription, PluginSource)> {
        let mut discovered = Vec::new();

        let entries = match std::fs::read_dir(path) {
            Ok(entries) => entries,
            Err(error) => {
                warn!(
                    "Failed to read plugins directory {}: {}",
                    path.display(),
                    error
                );
                return discovered;
            }
        };

        for entry in entries {
            let entry = match entry {
                Ok(entry) => entry,
                Err(error) => {
                    warn!("Failed to read plugins directory entry: {}", error);
                    continue;
                }
            };
            let sub_path = entry.path();

            if sub_path.is_dir() {
                let manifest_path = sub_path.join("plugin.yml");
                if !manifest_path.exists() {
                    continue;
                }
                match PluginDescription::from_file(&manifest_path) {
                    Ok(description) => {
                        discovered.push((description, PluginSource::Dir(sub_path)));
                    }
                    Err(error) => warn!(
                        "Failed to load plugin manifest {}: {}",
                        manifest_path.display(),
                        error
                    ),
                }
            } else if sub_path
                .extension()
                .and_then(|extension| extension.to_str())
                .map(|extension| extension.eq_ignore_ascii_case("phar"))
                .unwrap_or(false)
            {
                warn!(
                    "Ignoring phar plugin {} because the PHP runtime is unavailable",
                    sub_path.display()
                );
            }
        }

        discovered
    }

    /// Parses one manifest returned by PHP discovery.
    pub fn parse_discovered_manifest(
        item: &DiscoveredPluginManifest,
    ) -> Result<(PluginDescription, PluginSource), PluginError> {
        let description = PluginDescription::from_yaml_str(&item.manifest)?;
        let source = if item.kind.eq_ignore_ascii_case("phar") {
            PluginSource::phar(item.path.clone())
        } else {
            PluginSource::dir(item.path.clone())
        };
        Ok((description, source))
    }

    /// Topologically sorts plugins based on `depend`, `softdepend`, and `load_before`.
    pub fn resolve_dependencies(
        plugins: &[(PluginDescription, PathBuf)],
    ) -> Result<Vec<(PluginDescription, PathBuf)>, PluginError> {
        Self::resolve_order(plugins)
    }

    /// Same as [`PluginManager::resolve_dependencies`] for discovered sources.
    pub fn resolve_dependencies_with_sources(
        plugins: &[(PluginDescription, PluginSource)],
    ) -> Result<Vec<(PluginDescription, PluginSource)>, PluginError> {
        Self::resolve_order(plugins)
    }

    fn resolve_order<T: Clone>(
        plugins: &[(PluginDescription, T)],
    ) -> Result<Vec<(PluginDescription, T)>, PluginError> {
        let mut name_to_item: BTreeMap<String, (PluginDescription, T)> = BTreeMap::new();
        for (description, item) in plugins {
            name_to_item.insert(
                description.name.clone(),
                (description.clone(), item.clone()),
            );
        }

        // Validate mandatory hard dependencies exist.
        for (description, _) in name_to_item.values() {
            for dependency in &description.depend {
                if !name_to_item.contains_key(dependency) {
                    return Err(PluginError::MissingDependency(
                        dependency.clone(),
                        description.name.clone(),
                    ));
                }
            }
        }

        let mut in_degree: BTreeMap<String, usize> = BTreeMap::new();
        let mut graph: BTreeMap<String, Vec<String>> = BTreeMap::new();

        for name in name_to_item.keys() {
            in_degree.insert(name.clone(), 0);
            graph.insert(name.clone(), Vec::new());
        }

        // Edges:
        //   A depends on B       => B -> A
        //   A softdepends on B   => B -> A (only if B is present)
        //   A load_before B      => A -> B
        for (name, (description, _)) in &name_to_item {
            for dependency in &description.depend {
                add_edge(&mut graph, &mut in_degree, dependency, name);
            }
            for dependency in &description.softdepend {
                if name_to_item.contains_key(dependency) {
                    add_edge(&mut graph, &mut in_degree, dependency, name);
                }
            }
            for after in &description.load_before {
                if name_to_item.contains_key(after) {
                    add_edge(&mut graph, &mut in_degree, name, after);
                }
            }
        }

        let mut queue: VecDeque<String> = in_degree
            .iter()
            .filter(|(_, degree)| **degree == 0)
            .map(|(name, _)| name.clone())
            .collect();

        let mut sorted = Vec::with_capacity(name_to_item.len());
        while let Some(name) = queue.pop_front() {
            if let Some(item) = name_to_item.get(&name) {
                sorted.push(item.clone());
            }

            if let Some(neighbors) = graph.get(&name) {
                for neighbor in neighbors {
                    if let Some(degree) = in_degree.get_mut(neighbor) {
                        *degree = degree.saturating_sub(1);
                        if *degree == 0 {
                            queue.push_back(neighbor.clone());
                        }
                    }
                }
            }
        }

        if sorted.len() != name_to_item.len() {
            let remaining = in_degree
                .into_iter()
                .filter(|(_, degree)| *degree > 0)
                .map(|(name, _)| name)
                .collect::<Vec<_>>()
                .join(", ");
            return Err(PluginError::CircularDependency(remaining));
        }

        Ok(sorted)
    }
}

fn add_edge(
    graph: &mut BTreeMap<String, Vec<String>>,
    in_degree: &mut BTreeMap<String, usize>,
    from: &str,
    to: &str,
) {
    let neighbors = graph.entry(from.to_string()).or_default();
    if !neighbors.iter().any(|neighbor| neighbor == to) {
        neighbors.push(to.to_string());
        *in_degree.entry(to.to_string()).or_insert(0) += 1;
    }
}

fn split_command_line(command_line: &str) -> Option<(String, Vec<String>)> {
    let trimmed = command_line.trim().trim_start_matches('/');
    let mut parts = trimmed.split_whitespace();
    let command = parts.next()?.to_string();
    if command.is_empty() {
        return None;
    }
    Some((command, parts.map(str::to_string).collect()))
}

fn sender_to_json(sender: &CommandSender) -> serde_json::Value {
    match sender {
        CommandSender::Console => json!({
            "type": "console",
            "name": "CONSOLE"
        }),
        CommandSender::Player(player) => {
            // The PHP side reads the sender fields from the top level. Keep the
            // nested `player` key as well for forwards compatibility.
            let mut value = player.to_json();
            if let Some(object) = value.as_object_mut() {
                object.insert("type".to_string(), json!("player"));
                object.insert("name".to_string(), json!(player.name));
                object.insert("player".to_string(), player.to_json());
            }
            value
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_phar_discovery_manifest() {
        let item = DiscoveredPluginManifest {
            kind: "phar".to_string(),
            path: "plugins/Foo.phar".to_string(),
            manifest: "name: Foo\nversion: 1.0.0\nmain: Foo\\\\Main\napi: [5.0.0]\n".to_string(),
        };

        let (description, source) =
            PluginManager::parse_discovered_manifest(&item).expect("valid manifest");
        assert_eq!(description.name, "Foo");
        assert!(source.is_phar());
        assert!(source.root().starts_with("phar://"));
        assert!(source
            .main_file("Foo\\Main")
            .ends_with("/Foo.phar/src/Foo/Main.php"));
    }

    #[test]
    fn load_order_respects_soft_dependencies_and_load_before() {
        fn desc(
            name: &str,
            depend: &[&str],
            softdepend: &[&str],
            load_before: &[&str],
        ) -> PluginDescription {
            PluginDescription {
                name: name.to_string(),
                version: "1.0".to_string(),
                main: format!("{}\\Main", name),
                api: vec!["5.0.0".to_string()],
                author: None,
                authors: vec![],
                description: None,
                website: None,
                depend: depend.iter().map(|value| value.to_string()).collect(),
                softdepend: softdepend.iter().map(|value| value.to_string()).collect(),
                load_before: load_before.iter().map(|value| value.to_string()).collect(),
                commands: Default::default(),
                permissions: Default::default(),
            }
        }

        let plugins = vec![
            (
                desc("Consumer", &["Core"], &[], &[]),
                PluginSource::dir("plugins/Consumer"),
            ),
            (
                desc("Core", &[], &[], &[]),
                PluginSource::dir("plugins/Core"),
            ),
            (
                desc("Early", &[], &[], &["Core"]),
                PluginSource::dir("plugins/Early"),
            ),
        ];

        let order = PluginManager::resolve_dependencies_with_sources(&plugins)
            .unwrap()
            .into_iter()
            .map(|(description, _)| description.name)
            .collect::<Vec<_>>();

        let position = |name: &str| order.iter().position(|item| item == name).unwrap();
        assert!(position("Early") < position("Core"));
        assert!(position("Core") < position("Consumer"));
    }

    #[test]
    fn duplicate_dependencies_do_not_break_topological_sort() {
        let mut plugin = PluginDescription {
            name: "Consumer".to_string(),
            version: "1.0".to_string(),
            main: "Consumer\\Main".to_string(),
            api: vec![],
            author: None,
            authors: vec![],
            description: None,
            website: None,
            depend: vec!["Core".to_string(), "Core".to_string()],
            softdepend: vec![],
            load_before: vec![],
            commands: Default::default(),
            permissions: Default::default(),
        };
        let core = PluginDescription {
            name: "Core".to_string(),
            version: "1.0".to_string(),
            main: "Core\\Main".to_string(),
            api: vec![],
            author: None,
            authors: vec![],
            description: None,
            website: None,
            depend: vec![],
            softdepend: vec![],
            load_before: vec![],
            commands: Default::default(),
            permissions: Default::default(),
        };

        plugin.depend.shrink_to_fit();
        let order = PluginManager::resolve_dependencies(&[
            (plugin, PathBuf::from("plugins/Consumer")),
            (core, PathBuf::from("plugins/Core")),
        ])
        .unwrap();
        assert_eq!(order[0].0.name, "Core");
        assert_eq!(order[1].0.name, "Consumer");
    }
}
