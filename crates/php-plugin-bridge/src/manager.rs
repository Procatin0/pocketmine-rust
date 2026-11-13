use parking_lot::RwLock;
use serde_json::json;
use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::sync::Mutex as TokioMutex;
use tracing::{debug, info, warn};

use crate::command::{CommandMap, CommandSender};
use crate::context::PluginAction;
use crate::description::PluginDescription;
use crate::error::PluginError;
use crate::event::{Event, EventDispatcher};
use crate::host::PluginHost;
use crate::plugin::Plugin;
use crate::runtime::{DiscoveredPluginManifest, PhpPlugin, PhpWorker, PluginSource};

/// `plugin/PluginEnableOrder.php`: plugins enable in `STARTUP` before worlds
/// and in `POSTWORLD` after them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PluginEnableOrder {
    Startup,
    PostWorld,
}

impl PluginEnableOrder {
    pub fn from_str(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "startup" => Some(Self::Startup),
            "postworld" => Some(Self::PostWorld),
            _ => None,
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            Self::Startup => "startup",
            Self::PostWorld => "postworld",
        }
    }
}

impl PluginDescription {
    /// `PluginDescription::getOrder()` (`postworld` by default).
    pub fn enable_order(&self) -> PluginEnableOrder {
        self.load
            .as_deref()
            .and_then(PluginEnableOrder::from_str)
            .unwrap_or(PluginEnableOrder::PostWorld)
    }
}

/// Detailed information about a plugin that failed to load or enable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginFailure {
    pub name: String,
    pub reason: String,
}

/// Result of [`PluginManager::load_plugins_from_directory`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoadResult {
    pub loaded: usize,
    pub errors: usize,
    pub failures: Vec<PluginFailure>,
}

/// Result of [`PluginManager::enable_plugins_for`]: `true` when every
/// plugin in the phase enabled (`Server::enablePlugins` return value).
pub type EnableResult = bool;

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
    enable_failures: RwLock<Vec<PluginFailure>>,
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
            enable_failures: RwLock::new(Vec::new()),
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

    /// Failures encountered during the last `enable_plugins_for` execution.
    pub fn enable_failures(&self) -> Vec<PluginFailure> {
        self.enable_failures.read().clone()
    }

    pub fn current_tick(&self) -> u64 {
        self.current_tick.load(Ordering::SeqCst)
    }

    /// Initializes the PHP background worker.
    ///
    /// PM has no `info`/`warning` line for worker startup: failures are
    /// `DEBUG`-only here so the observable log matches `Server.php` (silent
    /// when PHP is missing; native discovery still runs).
    pub async fn init_runtime(&self) -> Result<(), PluginError> {
        match self.php_worker.start().await {
            Ok(true) => {}
            Ok(false) => {
                debug!("PHP runtime unavailable; PHP plugins will not be enabled");
            }
            Err(error) => {
                debug!("Failed to initialize PHP runtime: {}", error);
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
    /// Log parity with `PluginManager::loadPlugins` + `internalLoadPlugin`:
    /// `INFO "Loading {FullName}"` (`pocketmine.plugin.load`) before each
    /// `onLoad`, `CRITICAL "Could not load plugin '{name}': {details}"`
    /// (`pocketmine.plugin.loadError`) for duplicates, invalid manifests,
    /// unknown/circular dependencies and `onLoad` failures, and
    /// `WARNING "Plugin '{name}' uses spaces ..."` (`spacesDiscouraged`).
    /// Returns the loaded count; use [`Self::load_plugins_with_result`] for
    /// the `loadErrorCount` `Server.php` checks.
    pub async fn load_plugins_from_directory<P: AsRef<Path>>(
        &self,
        dir: P,
    ) -> Result<usize, PluginError> {
        let result = self.load_plugins_with_result(dir).await?;
        Ok(result.loaded)
    }

    /// Like [`Self::load_plugins_from_directory`] but also reports how many
    /// plugins failed (`$loadErrorCount` in `Server.php`). The caller is
    /// expected to log `EMERGENCY "Failed to load some plugins, ..."`
    /// (`pocketmine.plugin.someLoadErrors`) and stop the server when
    /// `errors > 0`.
    pub async fn load_plugins_with_result<P: AsRef<Path>>(
        &self,
        dir: P,
    ) -> Result<LoadResult, PluginError> {
        let path = dir.as_ref();
        if !path.exists() {
            std::fs::create_dir_all(path)?;
            *self.load_order.write() = Vec::new();
            return Ok(LoadResult {
                loaded: 0,
                errors: 0,
                failures: Vec::new(),
            });
        }

        let mut discovered: Vec<(PluginDescription, PluginSource)> = Vec::new();
        let mut load_error_count = 0usize;
        let mut failures: Vec<PluginFailure> = Vec::new();

        for item in self.discover_plugins_with_php(path) {
            match Self::parse_discovered_manifest(&item) {
                Ok(plugin) => discovered.push(plugin),
                Err(error) => {
                    // `loadError($file, invalidManifest($e->getMessage()))`
                    tracing::error!(
                        pm_level = "CRITICAL",
                        "Could not load plugin '{}': Invalid plugin manifest: {}",
                        item.path,
                        error
                    );
                    failures.push(PluginFailure {
                        name: item.path.clone(),
                        reason: format!("Invalid plugin manifest: {error}"),
                    });
                    load_error_count += 1;
                }
            }
        }

        if discovered.is_empty() {
            if !self.php_worker.is_available() {
                let native = Self::discover_plugin_directories(path, &mut load_error_count);
                discovered.extend(native);
            }
        } else {
            // Native fallback is only used when PHP found nothing; counting
            // is done inside `discover_plugin_directories` via the out-param.
            let _ = &discovered;
        }

        if discovered.is_empty() {
            // PM logs nothing for an empty plugin dir.
            *self.load_order.write() = Vec::new();
            return Ok(LoadResult {
                loaded: 0,
                errors: load_error_count,
                failures,
            });
        }

        // Duplicate detection mirrors `triagePlugins`: PM logs `duplicateError`
        // and drops the second entry. `resolve_order` keeps the last duplicate
        // in its map, so pre-detect duplicates here for an accurate count.
        let mut seen: HashSet<String> = self.plugins.read().keys().cloned().collect();
        let mut unique_discovered: Vec<(PluginDescription, PluginSource)> = Vec::new();
        for entry in discovered {
            let name = entry.0.name.clone();
            if seen.contains(&name) {
                tracing::error!(pm_level = "CRITICAL", "Could not load plugin '{name}': plugin exists");
                failures.push(PluginFailure {
                    name: name.clone(),
                    reason: "plugin exists".to_string(),
                });
                load_error_count += 1;
                continue;
            }
            if name.contains(' ') {
                warn!("Plugin '{name}' uses spaces in its name, this is discouraged");
            }
            seen.insert(name);
            unique_discovered.push(entry);
        }

        let ordered = match Self::resolve_dependencies_with_sources(&unique_discovered) {
            Ok(ordered) => ordered,
            Err(PluginError::MissingDependency(missing, requester)) => {
                tracing::error!(
                    pm_level = "CRITICAL",
                    "Could not load plugin '{requester}': Unknown dependency: {missing}"
                );
                failures.push(PluginFailure {
                    name: requester.clone(),
                    reason: format!("Unknown dependency: {missing}"),
                });
                return Ok(LoadResult {
                    loaded: 0,
                    errors: load_error_count + 1,
                    failures,
                });
            }
            Err(PluginError::CircularDependency(remaining)) => {
                for name in remaining.split(", ") {
                    let name = name.trim();
                    if !name.is_empty() {
                        tracing::error!(
                            pm_level = "CRITICAL",
                            "Could not load plugin '{name}': Circular dependency detected"
                        );
                        failures.push(PluginFailure {
                            name: name.to_string(),
                            reason: "Circular dependency detected".to_string(),
                        });
                    }
                }
                return Ok(LoadResult {
                    loaded: 0,
                    errors: load_error_count + 1,
                    failures,
                });
            }
            Err(other) => return Err(other),
        };
        let order: Vec<String> = ordered
            .iter()
            .map(|(description, _)| description.name.clone())
            .collect();
        debug!(
            "Calculated load order for {} plugins: {:?}",
            order.len(),
            order
        );
        *self.load_order.write() = order;

        let mut loaded_count = 0;
        for (description, source) in ordered {
            let name = description.name.clone();

            if self.plugins.read().contains_key(&name) {
                // `duplicateError`; already counted above for discovered
                // duplicates, but a native plugin could have collided.
                tracing::error!(pm_level = "CRITICAL", "Could not load plugin '{name}': plugin exists");
                failures.push(PluginFailure {
                    name: name.clone(),
                    reason: "plugin exists".to_string(),
                });
                load_error_count += 1;
                continue;
            }

            // `internalLoadPlugin`: `INFO "Loading {FullName}"`.
            info!("Loading {}", description.full_name());

            let plugin: Arc<TokioMutex<dyn Plugin>> = Arc::new(TokioMutex::new(PhpPlugin::new(
                description.clone(),
                source,
                Arc::clone(&self.php_worker),
            )));

            {
                let mut guard = plugin.lock().await;
                if let Err(error) = guard.on_load().await {
                    tracing::error!(pm_level = "CRITICAL", "Could not load plugin '{name}': {error}");
                    failures.push(PluginFailure {
                        name: name.clone(),
                        reason: error.to_string(),
                    });
                    load_error_count += 1;
                    continue;
                }
            }

            self.plugins.write().insert(name.clone(), plugin);
            self.descriptions.write().insert(name, description);
            loaded_count += 1;
        }

        Ok(LoadResult {
            loaded: loaded_count,
            errors: load_error_count,
            failures,
        })
    }

    /// Enables all loaded plugins in dependency order (`POSTWORLD` only,
    /// historically the only phase this method knew about).
    ///
    /// Prefer [`Self::enable_plugins_for`] with an explicit
    /// [`PluginEnableOrder`] so the `STARTUP`/`POSTWORLD` split matches
    /// `Server::enablePlugins`.
    pub async fn enable_plugins(&self) -> Result<bool, PluginError> {
        self.enable_plugins_for(PluginEnableOrder::PostWorld).await
    }

    /// Enables plugins whose `load:` phase matches `order`, in dependency
    /// order (`Server::enablePlugins(PluginEnableOrder $type)`).
    ///
    /// Logs `INFO "Enabling {FullName}"` (`pocketmine.plugin.enable`)
    /// before each `onEnable` and
    /// `CRITICAL "Could not enable plugin '{name}': {details}"`
    /// (`pocketmine.plugin.enableError`, `suicide` when the plugin disables
    /// itself) on failure. Returns `true` when every plugin in the phase
    /// enabled; the caller logs `EMERGENCY someEnableErrors` and stops the
    /// server when it is `false`.
    pub async fn enable_plugins_for(
        &self,
        order: PluginEnableOrder,
    ) -> Result<bool, PluginError> {
        let mut all_success = true;
        self.enable_failures.write().clear();
        for name in self.ordered_plugin_names() {
            let description = self.get_description(&name);
            let matches_phase = description
                .as_ref()
                .map(|d| d.enable_order() == order)
                .unwrap_or(order == PluginEnableOrder::PostWorld);
            if !matches_phase {
                continue;
            }
            let plugin = { self.plugins.read().get(&name).cloned() };
            let Some(plugin) = plugin else {
                continue;
            };

            let full_name = description
                .as_ref()
                .map(|d| d.full_name())
                .unwrap_or_else(|| name.clone());

            // `PluginManager::enablePlugin`: skip already-enabled plugins.
            let already_enabled = {
                let guard = plugin.lock().await;
                guard.is_enabled()
            };
            if already_enabled {
                continue;
            }

            info!("Enabling {full_name}");
            let mut guard = plugin.lock().await;
            match guard.on_enable().await {
                Ok(()) => {
                    if !guard.is_enabled() {
                        // The plugin disabled itself during onEnable (`suicide`).
                        tracing::error!(
                            pm_level = "CRITICAL",
                            "Could not enable plugin '{name}': Plugin disabled itself when attempting to enable it"
                        );
                        self.enable_failures.write().push(PluginFailure {
                            name: name.clone(),
                            reason: "Plugin disabled itself when attempting to enable it".to_string(),
                        });
                        all_success = false;
                        continue;
                    }
                    let events = guard.listener_events();
                    drop(guard);
                    self.record_plugin_listeners(&name, events);
                }
                Err(error) => {
                    tracing::error!(pm_level = "CRITICAL", "Could not enable plugin '{name}': {error}");
                    self.enable_failures.write().push(PluginFailure {
                        name: name.clone(),
                        reason: error.to_string(),
                    });
                    all_success = false;
                }
            }
        }

        self.refresh_listener_events();
        Ok(all_success)
    }

    /// Disables all loaded plugins in reverse dependency order.
    ///
    /// Logs `INFO "Disabling {FullName}"` (`pocketmine.plugin.disable`)
    /// before each `onDisable`. `onDisable` failures are `CRITICAL`-logged
    /// like `logException` paths in PM but do not stop the shutdown.
    pub async fn disable_plugins(&self) -> Result<(), PluginError> {
        let mut names = self.ordered_plugin_names();
        names.reverse();

        for name in names {
            let plugin = { self.plugins.read().get(&name).cloned() };
            let Some(plugin) = plugin else {
                continue;
            };

            let full_name = self
                .get_description(&name)
                .map(|d| d.full_name())
                .unwrap_or_else(|| name.clone());
            info!("Disabling {full_name}");

            {
                let mut guard = plugin.lock().await;
                if let Err(error) = guard.on_disable().await {
                    tracing::error!(pm_level = "CRITICAL", "Could not disable plugin '{name}': {error}");
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

        if !self.php_worker.is_shutting_down() && self.php_worker.is_available() {
            let ipc_started = std::time::Instant::now();
            let response = self.php_worker.request_sync(
                "dispatch_event",
                json!({
                    "event": event.name(),
                    "data": event.to_json()
                }),
            );
            let ipc_elapsed = ipc_started.elapsed();
            // The round-trip runs on the caller's (packet or tick) thread, so a
            // slow event is server lag: name it here, where the event is known.
            if ipc_elapsed > std::time::Duration::from_millis(20) {
                warn!(
                    "[lag-ipc] {} took {:.1}ms in the PHP worker",
                    event.name(),
                    (ipc_elapsed.as_secs_f64() * 10000.0).round() / 10.0
                );
            }
            match response {
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

        if !self.php_worker.is_shutting_down() && !self.php_worker.is_available() {
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

        // The shutdown sequence stops ticking before plugins are disabled;
        // late ticks (e.g. a blocking task already in flight) are no-ops so
        // they cannot restart the worker or spam warnings during shutdown.
        if self.php_worker.is_shutting_down() {
            return Ok(Vec::new());
        }

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
                debug!(
                    "PHP plugin discovery returned status '{}': {:?}",
                    response.status, response.message
                );
                Vec::new()
            }
            Err(error) => {
                debug!("PHP plugin discovery failed: {}", error);
                Vec::new()
            }
        }
    }

    /// Native fallback used when PHP is unavailable: directories with a
    /// `plugin.yml`. Phar archives require PHP's `phar://` support.
    ///
    /// Failures increment `load_error_count` with PM's
    /// `loadError($file, invalidManifest(...))` critical log.
    fn discover_plugin_directories(
        path: &Path,
        load_error_count: &mut usize,
    ) -> Vec<(PluginDescription, PluginSource)> {
        let mut discovered = Vec::new();

        let entries = match std::fs::read_dir(path) {
            Ok(entries) => entries,
            Err(error) => {
                debug!(
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
                    debug!("Failed to read plugins directory entry: {}", error);
                    continue;
                }
            };
            let sub_path = entry.path();

            if sub_path.is_dir() {
                let manifest_path = sub_path.join("plugin.yml");
                if !manifest_path.exists() {
                    continue;
                }
                match Self::from_file_validated(&manifest_path) {
                    Ok(description) => {
                        discovered.push((description, PluginSource::Dir(sub_path)));
                    }
                    Err(error) => {
                        tracing::error!(
                            "Could not load plugin '{}': Invalid plugin manifest: {}",
                            manifest_path.display(),
                            error
                        );
                        *load_error_count += 1;
                    }
                }
            } else if sub_path
                .extension()
                .and_then(|extension| extension.to_str())
                .map(|extension| extension.eq_ignore_ascii_case("phar"))
                .unwrap_or(false)
            {
                // `plugins/*.phar` can only be read through PHP's phar
                // wrapper; the native fallback intentionally does not try to
                // parse the archive itself (PM parity: the loader needs the
                // Phar extension). Warn instead of silently ignoring, but
                // never fail here (a phar without PHP must not stop the
                // server from starting).
                warn!(
                    "Skipping phar plugin {}: .phar plugins require the PHP runtime (phar support); install/enable PHP to load it",
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
        Self::validate_manifest_fields(&description, Some(&item.manifest))?;
        let source = if item.kind.eq_ignore_ascii_case("phar") {
            PluginSource::phar(item.path.clone())
        } else {
            PluginSource::dir(item.path.clone())
        };
        Ok((description, source))
    }

    /// Reads and validates a `plugin.yml` file (native discovery path).
    fn from_file_validated(manifest_path: &Path) -> Result<PluginDescription, PluginError> {
        let contents = std::fs::read_to_string(manifest_path)?;
        let description = PluginDescription::from_yaml_str(&contents)?;
        Self::validate_manifest_fields(&description, Some(&contents))?;
        Ok(description)
    }

    /// Extra `plugin.yml` validation beyond `PluginDescription::validate`
    /// (which covers `name`/`version`/`main`/`load`): `name` shape, `api`
    /// base versions, `commands` permission, `permissions` default and the
    /// raw `libraries` list. Mirrors the PHP `PluginDescription::fromArray`
    /// validation so both discovery paths reject the same manifests.
    /// `main` is intentionally not prefix-checked (bundled example uses
    /// `pocketmine\example\Main`).
    fn validate_manifest_fields(
        description: &PluginDescription,
        raw_manifest: Option<&str>,
    ) -> Result<(), PluginError> {
        if description.name.is_empty()
            || !description
                .name
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == ' ' || c == '_' || c == '.' || c == '-')
        {
            return Err(PluginError::InvalidManifest(description.name.clone(), "name"));
        }
        for api in &description.api {
            if !is_valid_base_version(api) {
                return Err(PluginError::InvalidManifest(description.name.clone(), "api"));
            }
        }
        for (command_name, command) in &description.commands {
            if command_name.trim().is_empty() {
                return Err(PluginError::InvalidManifest(description.name.clone(), "commands"));
            }
            match &command.permission {
                Some(permission) if !permission.trim().is_empty() => {}
                _ => {
                    return Err(PluginError::InvalidManifest(description.name.clone(), "commands"));
                }
            }
        }
        for permission in description.permissions.values() {
            let default = permission.default.trim().to_ascii_lowercase();
            if default != "true" && default != "false" && default != "op" && default != "notop" {
                return Err(PluginError::InvalidManifest(description.name.clone(), "permissions"));
            }
        }
        if let Some(raw) = raw_manifest {
            if let Ok(value) = serde_yaml::from_str::<serde_yaml::Value>(raw) {
                if let Some(libraries) = value.get("libraries") {
                    if !libraries.is_null() {
                        let Some(sequence) = libraries.as_sequence() else {
                            return Err(PluginError::InvalidManifest(
                                description.name.clone(),
                                "libraries",
                            ));
                        };
                        for item in sequence {
                            match item {
                                serde_yaml::Value::String(entry) if !entry.trim().is_empty() => {}
                                _ => {
                                    return Err(PluginError::InvalidManifest(
                                        description.name.clone(),
                                        "libraries",
                                    ));
                                }
                            }
                        }
                    }
                }
            }
        }
        Ok(())
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

/// PM `VersionString::isValidBaseVersion`: `X.Y.Z[-suffix]`.
fn is_valid_base_version(version: &str) -> bool {
    let base = match version.split_once('-') {
        Some((base, _suffix)) => base,
        None => version,
    };
    let parts: Vec<&str> = base.split('.').collect();
    parts.len() == 3
        && parts
            .iter()
            .all(|part| !part.is_empty() && part.chars().all(|c| c.is_ascii_digit()))
}

fn split_command_line(command_line: &str) -> Option<(String, Vec<String>)> {
    let trimmed = command_line.trim().trim_start_matches('/');
    let mut parts = trimmed.split_whitespace();
    let command = parts.next()?.to_string();
    if command.is_empty() {
        return None;
    }
    let raw_args: Vec<String> = parts.map(str::to_string).collect();
    Some((command, crate::command::merge_selector_tokens(&raw_args)))
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
                src_namespace_prefix: String::new(),
                load: None,
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
    fn enable_order_defaults_to_postworld() {
        fn desc_with_load(load: Option<&str>) -> PluginDescription {
            PluginDescription {
                name: "Foo".to_string(),
                version: "1.0".to_string(),
                main: "Foo\\Main".to_string(),
                api: vec![],
                author: None,
                authors: vec![],
                description: None,
                website: None,
                depend: vec![],
                softdepend: vec![],
                load_before: vec![],
                src_namespace_prefix: String::new(),
                load: load.map(|v| v.to_string()),
                commands: Default::default(),
                permissions: Default::default(),
            }
        }
        assert_eq!(
            desc_with_load(None).enable_order(),
            PluginEnableOrder::PostWorld
        );
        assert_eq!(
            desc_with_load(Some("startup")).enable_order(),
            PluginEnableOrder::Startup
        );
        assert_eq!(
            PluginEnableOrder::from_str("postworld"),
            Some(PluginEnableOrder::PostWorld)
        );
        assert!(desc_with_load(Some("startup")).is_startup());
        assert_eq!(
            desc_with_load(Some("1.0")).full_name(),
            "Foo v1.0"
        );
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
            src_namespace_prefix: String::new(),
            load: None,
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
            src_namespace_prefix: String::new(),
            load: None,
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

    #[test]
    fn rejects_invalid_api_commands_permissions_and_libraries() {
        fn parse(manifest: &str) -> Result<PluginDescription, crate::error::PluginError> {
            let item = DiscoveredPluginManifest {
                kind: "dir".to_string(),
                path: "plugins/Foo".to_string(),
                manifest: manifest.to_string(),
            };
            PluginManager::parse_discovered_manifest(&item).map(|(description, _)| description)
        }

        let valid = "name: Foo\nversion: 1.0.0\nmain: Foo\\\\Main\napi: [5.0.0]\ncommands:\n  hello:\n    permission: foo.command.hello\npermissions:\n  foo.command.hello:\n    default: op\n";
        assert!(parse(valid).is_ok(), "valid manifest parses");

        let bad_api = "name: Foo\nversion: 1.0.0\nmain: Foo\\\\Main\napi: [bad]\n";
        assert!(parse(bad_api).is_err(), "invalid api rejected");

        let no_permission = "name: Foo\nversion: 1.0.0\nmain: Foo\\\\Main\napi: [5.0.0]\ncommands:\n  hello:\n    description: Hi\n";
        assert!(parse(no_permission).is_err(), "command without permission rejected");

        let bad_default = "name: Foo\nversion: 1.0.0\nmain: Foo\\\\Main\napi: [5.0.0]\npermissions:\n  foo.bar:\n    default: bogus\n";
        assert!(parse(bad_default).is_err(), "invalid permission default rejected");

        let valid_libraries = "name: Foo\nversion: 1.0.0\nmain: Foo\\\\Main\napi: [5.0.0]\nlibraries: [lib-a, lib-b]\n";
        assert!(parse(valid_libraries).is_ok(), "string libraries accepted");

        let bad_libraries = "name: Foo\nversion: 1.0.0\nmain: Foo\\\\Main\napi: [5.0.0]\nlibraries: [[nested]]\n";
        assert!(parse(bad_libraries).is_err(), "non-string libraries rejected");
    }

    #[test]
    fn load_result_tracks_failures() {
        let failure = PluginFailure {
            name: "FaultyPlugin".to_string(),
            reason: "Parse error: syntax error in Foo.php".to_string(),
        };
        let result = LoadResult {
            loaded: 1,
            errors: 1,
            failures: vec![failure.clone()],
        };
        assert_eq!(result.loaded, 1);
        assert_eq!(result.errors, 1);
        assert_eq!(result.failures.len(), 1);
        assert_eq!(result.failures[0].name, "FaultyPlugin");
        assert_eq!(result.failures[0].reason, "Parse error: syntax error in Foo.php");
    }
}
