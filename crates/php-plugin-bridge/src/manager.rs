use parking_lot::RwLock;
use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::Mutex as TokioMutex;
use tracing::{error, info, warn};

use crate::command::{CommandMap, CommandSender};
use crate::context::PluginAction;
use crate::description::PluginDescription;
use crate::error::PluginError;
use crate::event::{Event, EventDispatcher};
use crate::plugin::Plugin;
use crate::runtime::{PhpPlugin, PhpWorker};

pub struct PluginManager {
    plugins: RwLock<HashMap<String, Arc<TokioMutex<dyn Plugin>>>>,
    descriptions: RwLock<HashMap<String, PluginDescription>>,
    dispatcher: Arc<EventDispatcher>,
    command_map: Arc<CommandMap>,
    php_worker: Arc<PhpWorker>,
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

    pub fn plugin_count(&self) -> usize {
        self.plugins.read().len()
    }

    pub fn get_description(&self, name: &str) -> Option<PluginDescription> {
        self.descriptions.read().get(name).cloned()
    }

    /// Initializes the PHP background worker.
    pub async fn init_runtime(&self) -> Result<(), PluginError> {
        let _ = self.php_worker.start().await?;
        Ok(())
    }

    /// Registers a native Rust plugin manually.
    pub async fn register_native_plugin<P: Plugin + 'static>(&self, plugin: P) -> Result<(), PluginError> {
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

    /// Scans a directory (e.g. `plugins/`) and loads all plugins respecting dependencies.
    pub async fn load_plugins_from_directory<P: AsRef<Path>>(&self, dir: P) -> Result<usize, PluginError> {
        let path = dir.as_ref();
        if !path.exists() {
            std::fs::create_dir_all(path)?;
            return Ok(0);
        }

        let mut discovered = Vec::new();
        for entry in std::fs::read_dir(path)? {
            let entry = entry?;
            let sub_path = entry.path();
            if sub_path.is_dir() {
                let manifest_path = sub_path.join("plugin.yml");
                if manifest_path.exists() {
                    match PluginDescription::from_file(&manifest_path) {
                        Ok(desc) => {
                            discovered.push((desc, sub_path));
                        }
                        Err(e) => {
                            warn!("Failed to load plugin manifest in {}: {}", sub_path.display(), e);
                        }
                    }
                }
            }
        }

        let load_order = Self::resolve_dependencies(&discovered)?;
        info!("Calculated load order for {} plugins: {:?}", load_order.len(), load_order.iter().map(|(d, _)| &d.name).collect::<Vec<_>>());

        let mut loaded_count = 0;
        for (desc, plugin_dir) in load_order {
            let name = desc.name.clone();
            let php_plugin = PhpPlugin::new(desc.clone(), plugin_dir, Arc::clone(&self.php_worker));
            let plugin_arc = Arc::new(TokioMutex::new(php_plugin));

            {
                let mut p = plugin_arc.lock().await;
                if let Err(e) = p.on_load().await {
                    error!("Error during onLoad for plugin '{}': {}", name, e);
                    continue;
                }
            }

            self.plugins.write().insert(name.clone(), plugin_arc);
            self.descriptions.write().insert(name, desc);
            loaded_count += 1;
        }

        Ok(loaded_count)
    }

    /// Enables all loaded plugins in order.
    pub async fn enable_plugins(&self) -> Result<(), PluginError> {
        let plugins: Vec<(String, Arc<TokioMutex<dyn Plugin>>)> = {
            let map = self.plugins.read();
            map.iter().map(|(k, v)| (k.clone(), Arc::clone(v))).collect()
        };

        for (name, plugin) in plugins {
            let mut p = plugin.lock().await;
            if let Err(e) = p.on_enable().await {
                error!("Error enabling plugin '{}': {}", name, e);
            }
        }

        Ok(())
    }

    /// Disables all loaded plugins.
    pub async fn disable_plugins(&self) -> Result<(), PluginError> {
        let plugins: Vec<(String, Arc<TokioMutex<dyn Plugin>>)> = {
            let map = self.plugins.read();
            map.iter().map(|(k, v)| (k.clone(), Arc::clone(v))).collect()
        };

        for (name, plugin) in plugins.into_iter().rev() {
            let mut p = plugin.lock().await;
            if let Err(e) = p.on_disable().await {
                error!("Error disabling plugin '{}': {}", name, e);
            }
        }

        Ok(())
    }

    /// Dispatches an event through both native listeners and the PHP worker.
    pub async fn dispatch_event(&self, event: &mut dyn Event) -> Result<Vec<PluginAction>, PluginError> {
        // 1. Dispatch through registered native listeners
        let mut actions = self.dispatcher.dispatch(event)?;

        // 2. Dispatch through PHP worker if available
        let php_actions = self.php_worker.dispatch_event(event).await?;
        actions.extend(php_actions);

        Ok(actions)
    }

    /// Executes a command via the command map.
    pub fn execute_command(&self, sender: &CommandSender, command_line: &str) -> Result<Option<Vec<PluginAction>>, PluginError> {
        self.command_map.execute(sender, command_line)
    }

    /// Topologically sorts plugins based on `depend`, `softdepend`, and `load_before`.
    pub fn resolve_dependencies(
        plugins: &[(PluginDescription, PathBuf)],
    ) -> Result<Vec<(PluginDescription, PathBuf)>, PluginError> {
        let mut name_to_item: HashMap<String, (PluginDescription, PathBuf)> = HashMap::new();
        let mut in_degree: HashMap<String, usize> = HashMap::new();
        let mut graph: HashMap<String, Vec<String>> = HashMap::new();

        for (desc, path) in plugins {
            name_to_item.insert(desc.name.clone(), (desc.clone(), path.clone()));
            in_degree.insert(desc.name.clone(), 0);
            graph.insert(desc.name.clone(), Vec::new());
        }

        // Validate mandatory hard dependencies exist
        for (desc, _) in plugins {
            for dep in &desc.depend {
                if !name_to_item.contains_key(dep) {
                    return Err(PluginError::MissingDependency(dep.clone(), desc.name.clone()));
                }
            }
        }

        // Build edges:
        // A depends on B => B must load before A (B -> A)
        // A load_before B => A must load before B (A -> B)
        for (desc, _) in plugins {
            let u = &desc.name;

            // Depend
            for dep in &desc.depend {
                if name_to_item.contains_key(dep) {
                    graph.entry(dep.clone()).or_default().push(u.clone());
                    *in_degree.entry(u.clone()).or_insert(0) += 1;
                }
            }

            // Softdepend (only if present)
            for sdep in &desc.softdepend {
                if name_to_item.contains_key(sdep) {
                    graph.entry(sdep.clone()).or_default().push(u.clone());
                    *in_degree.entry(u.clone()).or_insert(0) += 1;
                }
            }

            // Load before
            for before in &desc.load_before {
                if name_to_item.contains_key(before) {
                    graph.entry(u.clone()).or_default().push(before.clone());
                    *in_degree.entry(before.clone()).or_insert(0) += 1;
                }
            }
        }

        // Kahn's algorithm
        let mut queue = VecDeque::new();
        for (name, deg) in &in_degree {
            if *deg == 0 {
                queue.push_back(name.clone());
            }
        }

        let mut sorted = Vec::new();
        while let Some(u) = queue.pop_front() {
            if let Some(item) = name_to_item.get(&u) {
                sorted.push(item.clone());
            }
            if let Some(neighbors) = graph.get(&u) {
                for v in neighbors {
                    if let Some(deg) = in_degree.get_mut(v) {
                        *deg -= 1;
                        if *deg == 0 {
                            queue.push_back(v.clone());
                        }
                    }
                }
            }
        }

        if sorted.len() != plugins.len() {
            let remaining = in_degree
                .into_iter()
                .filter(|(_, deg)| *deg > 0)
                .map(|(k, _)| k)
                .collect::<Vec<_>>()
                .join(", ");
            return Err(PluginError::CircularDependency(remaining));
        }

        Ok(sorted)
    }
}
