use parking_lot::{Mutex, RwLock};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use std::collections::VecDeque;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender, SyncSender};
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, warn};

use crate::context::PluginAction;
use crate::description::PluginDescription;
use crate::error::PluginError;
use crate::event::Event;
use crate::host::PluginHost;
use crate::plugin::Plugin;
use crate::runtime::bootstrap;

const PING_TIMEOUT: Duration = Duration::from_secs(15);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
/// Upper bound waiting for the PHP worker thread (and its child process) to
/// exit during shutdown. After this, the child is force-killed and shutdown
/// proceeds — PM `forceShutdown` must never hang on a wedged thread.
const WORKER_SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(5);

/// Manifest returned by PHP's `discover_plugins` action.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct DiscoveredPluginManifest {
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub path: String,
    #[serde(default)]
    pub manifest: String,
}

/// A single response line from the PHP worker.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct IpcResponse {
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub message: Option<String>,
    #[serde(default)]
    pub cancelled: bool,
    #[serde(default)]
    pub event: Option<Value>,
    #[serde(default)]
    pub actions: Vec<PluginAction>,
    #[serde(default)]
    pub handled: bool,
    #[serde(default)]
    pub plugins: Vec<DiscoveredPluginManifest>,
    #[serde(default)]
    pub commands: Vec<String>,
    #[serde(default)]
    pub listener_events: Vec<String>,
    #[serde(default)]
    pub listeners: usize,
    #[serde(default)]
    pub tick: u64,
    #[serde(default)]
    pub pending_tasks: u64,
    #[serde(default)]
    pub data: Option<Value>,
}

struct WorkerRequest {
    action: String,
    extra: Value,
    reply: SyncSender<Result<IpcResponse, PluginError>>,
}

/// Cloneable handle shared with blocking tasks and the worker thread.
#[derive(Clone)]
struct WorkerHandle {
    tx: Arc<Mutex<Option<Sender<WorkerRequest>>>>,
    available: Arc<AtomicBool>,
    host: Arc<RwLock<Option<Arc<dyn PluginHost>>>>,
    runtime_dir: Arc<Mutex<Option<PathBuf>>>,
    worker_thread: Arc<Mutex<Option<std::thread::JoinHandle<()>>>>,
    /// Set when the server shutdown sequence has started. Once set, no new
    /// PHP process is ever spawned: requests against a dead worker fail fast
    /// instead of restarting an empty runtime mid-shutdown (which showed up
    /// as a second `[PHP/RUNTIME] Runtime ready` line during `disablePlugins`
    /// and silently skipped the remaining plugins' `onDisable`).
    shutdown: Arc<AtomicBool>,
    /// PID of the current PHP child, kept so a wedged worker can be
    /// force-killed instead of blocking shutdown forever.
    child_pid: Arc<Mutex<Option<u32>>>,
}

impl WorkerHandle {
    fn is_available(&self) -> bool {
        self.available.load(Ordering::SeqCst)
    }

    fn mark_unavailable(&self) {
        self.available.store(false, Ordering::SeqCst);
        let _ = self.tx.lock().take();
    }

    fn start_sync(&self) -> Result<bool, PluginError> {
        // Once shutdown starts the runtime is never restarted. This also
        // keeps the tick loop (which keeps calling `tick_sync` until the
        // runtime drops) from respawning a fresh, empty PHP worker while
        // plugins are being disabled.
        if self.shutdown.load(Ordering::SeqCst) {
            return Ok(false);
        }

        if self.is_available() && self.tx.lock().is_some() {
            return Ok(true);
        }

        // Serialize concurrent startup attempts.
        let mut tx_guard = self.tx.lock();

        if self.is_available() && tx_guard.is_some() {
            return Ok(true);
        }

        *tx_guard = None;
        self.available.store(false, Ordering::SeqCst);

        // Never leave a stale PHP process behind: a previous worker that
        // timed out may still be alive (its thread is blocked reading a
        // wedged stdout). Kill it before spawning its replacement.
        if let Some(pid) = self.child_pid.lock().take() {
            debug!("Stopping stale PHP worker process (pid {pid}) before restart");
            kill_process(pid);
        }

        let runtime_dir = bootstrap::default_runtime_dir();
        let bootstrap_path = bootstrap::write_runtime(&runtime_dir)?;
        *self.runtime_dir.lock() = Some(bootstrap_path.clone());

        let php = std::env::var("POCKETMINE_PHP").unwrap_or_else(|_| "php".to_string());
        let mut child = match std::process::Command::new(&php)
            .arg(&bootstrap_path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
        {
            Ok(child) => child,
            Err(error) => {
                // Missing PHP is not fatal: the manager falls back to native
                // directory discovery and the server keeps running.
                warn!(
                    "PHP executable '{}' could not be started ({}); PHP plugins disabled",
                    php, error
                );
                return Ok(false);
            }
        };

        let stdin = match child.stdin.take() {
            Some(stdin) => stdin,
            None => {
                let _ = child.kill();
                return Err(PluginError::PhpRuntime(
                    "Failed to capture PHP worker stdin".to_string(),
                ));
            }
        };
        let stdout = match child.stdout.take() {
            Some(stdout) => stdout,
            None => {
                let _ = child.kill();
                return Err(PluginError::PhpRuntime(
                    "Failed to capture PHP worker stdout".to_string(),
                ));
            }
        };

        let stderr = child.stderr.take();
        let stderr_lines = Arc::new(Mutex::new(VecDeque::<String>::with_capacity(64)));
        if let Some(stderr) = stderr {
            let buf_clone = Arc::clone(&stderr_lines);
            std::thread::Builder::new()
                .name("php-worker-stderr".to_string())
                .spawn(move || {
                    let reader = BufReader::new(stderr);
                    for line in reader.lines() {
                        let Ok(line) = line else { break };
                        let trimmed = line.trim();
                        if trimmed.is_empty() {
                            continue;
                        }
                        if trimmed.starts_with("[PHP/RUNTIME]") || trimmed.starts_with("[PHP/BOOTSTRAP]") {
                            debug!("{trimmed}");
                        } else if trimmed.contains("Fatal error")
                            || trimmed.contains("Parse error")
                            || trimmed.contains("Compile error")
                            || trimmed.starts_with("[PHP/FATAL]")
                        {
                            tracing::error!(pm_level = "CRITICAL", "{trimmed}");
                        } else if trimmed.contains("Warning") || trimmed.contains("Notice") {
                            tracing::warn!("{trimmed}");
                        } else {
                            debug!("{trimmed}");
                        }
                        let mut buf = buf_clone.lock();
                        if buf.len() >= 64 {
                            buf.pop_front();
                        }
                        buf.push_back(trimmed.to_string());
                    }
                })
                .ok();
        }

        let (sender, receiver) = mpsc::channel::<WorkerRequest>();
        let host = Arc::clone(&self.host);
        *self.child_pid.lock() = Some(child.id());
        let stderr_lines_for_worker = Arc::clone(&stderr_lines);
        let join_handle = match std::thread::Builder::new()
            .name("php-plugin-worker".to_string())
            .spawn(move || run_worker(child, stdin, stdout, receiver, host, stderr_lines_for_worker))
        {
            Ok(handle) => handle,
            Err(error) => {
                return Err(PluginError::PhpRuntime(format!(
                    "Failed to spawn PHP worker thread: {}",
                    error
                )));
            }
        };

        *tx_guard = Some(sender.clone());
        self.available.store(true, Ordering::SeqCst);
        drop(tx_guard);

        // Keep the join handle so shutdown can wait for the worker thread.
        *self.worker_thread.lock() = Some(join_handle);

        // Handshake: make sure the PHP loop is alive before declaring the
        // runtime usable.
        let (reply_tx, reply_rx) = mpsc::sync_channel(1);
        let ping = WorkerRequest {
            action: "ping".to_string(),
            extra: Value::Object(Map::new()),
            reply: reply_tx,
        };

        if sender.send(ping).is_err() {
            self.mark_unavailable();
            return Ok(false);
        }

        match reply_rx.recv_timeout(PING_TIMEOUT) {
            Ok(Ok(response)) if response.status == "ok" => {
                debug!("PHP plugin runtime worker started successfully");
                Ok(true)
            }
            Ok(Ok(response)) => {
                warn!(
                    "PHP worker ping failed with status '{}': {:?}",
                    response.status, response.message
                );
                self.mark_unavailable();
                Ok(false)
            }
            Ok(Err(error)) => {
                warn!("PHP worker ping failed: {}", error);
                self.mark_unavailable();
                Ok(false)
            }
            Err(error) => {
                warn!("PHP worker ping timed out: {}", error);
                self.mark_unavailable();
                Ok(false)
            }
        }
    }

    fn request_sync(&self, action: &str, extra: Value) -> Result<IpcResponse, PluginError> {
        if !self.is_available() && !self.start_sync()? {
            return Err(PluginError::PhpRuntime(
                "PHP runtime is not available".to_string(),
            ));
        }

        let sender = match self.tx.lock().clone() {
            Some(sender) => sender,
            None => {
                self.available.store(false, Ordering::SeqCst);
                return Err(PluginError::PhpRuntime(
                    "PHP worker channel is closed".to_string(),
                ));
            }
        };

        let (reply_tx, reply_rx) = mpsc::sync_channel(1);
        let request = WorkerRequest {
            action: action.to_string(),
            extra,
            reply: reply_tx,
        };

        if sender.send(request).is_err() {
            self.mark_unavailable();
            return Err(PluginError::PhpRuntime(
                "PHP worker thread is no longer running".to_string(),
            ));
        }

        match reply_rx.recv_timeout(REQUEST_TIMEOUT) {
            Ok(Ok(response)) => Ok(response),
            Ok(Err(error)) => {
                self.mark_unavailable();
                Err(error)
            }
            Err(error) => {
                self.mark_unavailable();
                Err(PluginError::PhpRuntime(format!(
                    "Timeout waiting for PHP action '{}': {}",
                    action, error
                )))
            }
        }
    }
}

/// The IPC bridge managing the persistent PHP worker subprocess.
#[derive(Clone)]
pub struct PhpWorker {
    tx: Arc<Mutex<Option<Sender<WorkerRequest>>>>,
    available: Arc<AtomicBool>,
    host: Arc<RwLock<Option<Arc<dyn PluginHost>>>>,
    runtime_dir: Arc<Mutex<Option<PathBuf>>>,
    /// Join handle for the PHP worker thread, kept so shutdown can wait for it.
    worker_thread: Arc<Mutex<Option<std::thread::JoinHandle<()>>>>,
    /// See `WorkerHandle::shutdown`.
    shutdown: Arc<AtomicBool>,
    /// See `WorkerHandle::child_pid`.
    child_pid: Arc<Mutex<Option<u32>>>,
}

impl Default for PhpWorker {
    fn default() -> Self {
        Self::new()
    }
}

impl PhpWorker {
    pub fn new() -> Self {
        Self {
            tx: Arc::new(Mutex::new(None)),
            available: Arc::new(AtomicBool::new(false)),
            host: Arc::new(RwLock::new(None)),
            runtime_dir: Arc::new(Mutex::new(None)),
            worker_thread: Arc::new(Mutex::new(None)),
            shutdown: Arc::new(AtomicBool::new(false)),
            child_pid: Arc::new(Mutex::new(None)),
        }
    }

    fn handle(&self) -> WorkerHandle {
        WorkerHandle {
            tx: Arc::clone(&self.tx),
            available: Arc::clone(&self.available),
            host: Arc::clone(&self.host),
            runtime_dir: Arc::clone(&self.runtime_dir),
            worker_thread: Arc::clone(&self.worker_thread),
            shutdown: Arc::clone(&self.shutdown),
            child_pid: Arc::clone(&self.child_pid),
        }
    }



    /// Installs the host used for synchronous PHP -> Rust calls.
    pub fn set_host(&self, host: Arc<dyn PluginHost>) {
        *self.host.write() = Some(host);
    }

    pub fn is_available(&self) -> bool {
        self.available.load(Ordering::SeqCst)
    }

    /// Path of the generated `bootstrap.php`, if the runtime was started.
    pub fn runtime_dir(&self) -> Option<PathBuf> {
        self.runtime_dir.lock().clone()
    }

    /// Attempts to spawn the PHP worker process and ping it.
    ///
    /// Returns `Ok(false)` when PHP is not installed or the runtime cannot be
    /// reached; callers should treat that as "PHP plugins unavailable".
    pub async fn start(&self) -> Result<bool, PluginError> {
        let handle = self.handle();
        tokio::task::spawn_blocking(move || handle.start_sync())
            .await
            .map_err(|error| {
                PluginError::PhpRuntime(format!("PHP worker startup task failed: {}", error))
            })?
    }

    /// Synchronous startup used by `request_sync` and `init_runtime` internals.
    pub fn start_sync(&self) -> Result<bool, PluginError> {
        self.handle().start_sync()
    }

    /// Sends a request to PHP and waits for the final response.
    ///
    /// If the worker is not running this attempts one restart. Timeouts and
    /// broken pipes mark the runtime as unavailable.
    pub fn request_sync(&self, action: &str, extra: Value) -> Result<IpcResponse, PluginError> {
        self.handle().request_sync(action, extra)
    }

    /// Async wrapper around [`PhpWorker::request_sync`].
    pub async fn send_command(
        &self,
        action: &str,
        extra: Value,
    ) -> Result<IpcResponse, PluginError> {
        let handle = self.handle();
        let action = action.to_string();
        tokio::task::spawn_blocking(move || handle.request_sync(&action, extra))
            .await
            .map_err(|error| {
                PluginError::PhpRuntime(format!("PHP worker request task failed: {}", error))
            })?
    }

    /// Dispatches an event to PHP and applies the mutated snapshot back.
    pub async fn dispatch_event(
        &self,
        event: &mut dyn Event,
    ) -> Result<Vec<PluginAction>, PluginError> {
        if !self.is_available() {
            return Ok(Vec::new());
        }

        let response = self
            .send_command(
                "dispatch_event",
                json!({
                    "event": event.name(),
                    "data": event.to_json()
                }),
            )
            .await?;

        if response.status != "ok" && response.status != "offline" {
            warn!(
                "PHP dispatch of '{}' returned status '{}': {:?}",
                event.name(),
                response.status,
                response.message
            );
        }

        if let Some(value) = &response.event {
            event.apply_json(value);
        }
        if response.cancelled && event.is_cancellable() {
            event.set_cancelled(true);
        }

        Ok(response.actions)
    }

    /// Flags the worker as shutting down WITHOUT stopping it: from now on no
    /// new PHP process is spawned (dead-worker requests fail fast), but
    /// in-flight requests keep working. The shutdown sequence in `main.rs`
    /// calls this before `disable_plugins()` so a plugin that kills the
    /// worker cannot trigger a mid-shutdown restart of an empty runtime.
    pub fn begin_shutdown(&self) {
        self.shutdown.store(true, Ordering::SeqCst);
    }

    /// Whether [`Self::begin_shutdown`] has been called.
    pub fn is_shutting_down(&self) -> bool {
        self.shutdown.load(Ordering::SeqCst)
    }

    /// Asks the PHP worker to exit and waits for the worker thread to finish.
    ///
    /// Sends a `shutdown` request to the worker, then joins the worker thread
    /// so the PHP subprocess has time to exit cleanly. The wait is bounded:
    /// if the thread does not finish within [`WORKER_SHUTDOWN_TIMEOUT`] the
    /// PHP child is force-killed and the function returns anyway — shutdown
    /// must never hang forever on a wedged worker (PM `forceShutdown` parity:
    /// after this the server exits).
    pub fn shutdown_sync(&self) {
        self.begin_shutdown();
        self.available.store(false, Ordering::SeqCst);

        // Send the shutdown request so the worker loop breaks.
        if let Some(sender) = self.tx.lock().take() {
            let (reply, _receiver) = mpsc::sync_channel(1);
            let _ = sender.send(WorkerRequest {
                action: "shutdown".to_string(),
                extra: Value::Object(Map::new()),
                reply,
            });
        }

        // Wait (bounded) for the worker thread, force-killing the PHP child
        // when the worker does not exit in time.
        if let Some(handle) = self.worker_thread.lock().take() {
            join_with_timeout(
                handle,
                WORKER_SHUTDOWN_TIMEOUT,
                Arc::clone(&self.child_pid),
            );
        }

        // Final cleanup: reap/kill the PHP child if it is still around.
        if let Some(pid) = self.child_pid.lock().take() {
            kill_process(pid);
        }
    }
}

/// Joins `handle` waiting at most `timeout`, then kills the PHP child (via
/// `child_pid`) so a wedged worker thread — e.g. blocked reading a hung
/// stdout — unblocks and can be reaped by its detached joiner.
fn join_with_timeout(
    handle: std::thread::JoinHandle<()>,
    timeout: Duration,
    child_pid: Arc<Mutex<Option<u32>>>,
) {
    let (done_tx, done_rx) = mpsc::channel();
    std::thread::Builder::new()
        .name("php-plugin-worker-join".to_string())
        .spawn(move || {
            let _ = handle.join();
            let _ = done_tx.send(());
        })
        .ok();

    match done_rx.recv_timeout(timeout) {
        Ok(()) => {}
        Err(_) => {
            warn!(
                "PHP worker thread did not exit within {:?}; force-killing the PHP process",
                timeout
            );
            if let Some(pid) = child_pid.lock().take() {
                debug!("Force-killing PHP worker process (pid {pid})");
                kill_process(pid);
            }
        }
    }
}

/// Best-effort force-kill of a PHP worker process by PID.
fn kill_process(pid: u32) {
    #[cfg(windows)]
    let _ = std::process::Command::new("taskkill")
        .args(["/PID", &pid.to_string(), "/F"])
        .output();
    #[cfg(not(windows))]
    let _ = std::process::Command::new("kill")
        .args(["-9", &pid.to_string()])
        .output();
}

impl Drop for PhpWorker {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::SeqCst);
        self.available.store(false, Ordering::SeqCst);
        if let Some(sender) = self.tx.lock().take() {
            let (reply, _receiver) = mpsc::sync_channel(1);
            let _ = sender.send(WorkerRequest {
                action: "shutdown".to_string(),
                extra: Value::Object(Map::new()),
                reply,
            });
        }
    }
}

fn run_worker(
    mut child: Child,
    mut stdin: ChildStdin,
    stdout: ChildStdout,
    receiver: Receiver<WorkerRequest>,
    host: Arc<RwLock<Option<Arc<dyn PluginHost>>>>,
    stderr_lines: Arc<Mutex<VecDeque<String>>>,
) {
    let mut reader = BufReader::new(stdout);

    while let Ok(request) = receiver.recv() {
        let mut payload = Map::new();
        payload.insert("action".to_string(), Value::String(request.action.clone()));
        if let Value::Object(extra) = request.extra {
            for (key, value) in extra {
                payload.insert(key, value);
            }
        }

        let line = match serde_json::to_string(&Value::Object(payload)) {
            Ok(line) => line,
            Err(error) => {
                let _ = request.reply.send(Err(PluginError::PhpRuntime(format!(
                    "Failed to encode PHP request: {}",
                    error
                ))));
                continue;
            }
        };

        if let Err(error) = write_json_line(&mut stdin, &line) {
            let _ = request.reply.send(Err(error));
            break;
        }

        if request.action == "shutdown" {
            let _ = request.reply.send(Ok(IpcResponse {
                status: "ok".to_string(),
                ..IpcResponse::default()
            }));
            break;
        }

        let result = read_response(&mut reader, &mut stdin, &host, &stderr_lines);
        let failed = result.is_err();
        let _ = request.reply.send(result);

        if failed {
            break;
        }
    }

    // Closing stdin lets a well-behaved PHP loop exit on EOF. The wait for
    // the child is bounded so a wedged worker cannot hang shutdown forever;
    // after `WORKER_SHUTDOWN_TIMEOUT` the child is force-killed.
    drop(stdin);
    let deadline = std::time::Instant::now() + WORKER_SHUTDOWN_TIMEOUT;
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) => {
                if std::time::Instant::now() >= deadline {
                    let _ = child.kill();
                }
                std::thread::sleep(Duration::from_millis(50));
            }
            Err(_) => break,
        }
    }
    let _ = child.wait();
}

fn read_response(
    reader: &mut BufReader<ChildStdout>,
    stdin: &mut ChildStdin,
    host: &RwLock<Option<Arc<dyn PluginHost>>>,
    stderr_lines: &Arc<Mutex<VecDeque<String>>>,
) -> Result<IpcResponse, PluginError> {
    loop {
        let mut line = String::new();
        let bytes = reader.read_line(&mut line)?;
        if bytes == 0 {
            // Give stderr reader thread a short window to capture trailing error lines.
            let mut captured = None;
            for _ in 0..5 {
                let buf = stderr_lines.lock();
                let relevant: Vec<String> = buf
                    .iter()
                    .filter(|l| !l.starts_with("[PHP/RUNTIME]") && !l.starts_with("[PHP/BOOTSTRAP]"))
                    .cloned()
                    .collect();
                if !relevant.is_empty() {
                    captured = Some(relevant.join(" | "));
                    break;
                }
                drop(buf);
                std::thread::sleep(Duration::from_millis(15));
            }
            if captured.is_none() {
                let buf = stderr_lines.lock();
                if !buf.is_empty() {
                    captured = Some(buf.iter().cloned().collect::<Vec<_>>().join(" | "));
                }
            }
            let details = match captured {
                Some(err) => format!("PHP worker terminated: {}", err),
                None => "PHP worker closed stdout unexpectedly".to_string(),
            };
            return Err(PluginError::PhpRuntime(details));
        }

        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let value: Value = match serde_json::from_str(trimmed) {
            Ok(value) => value,
            Err(error) => {
                return Err(PluginError::PhpRuntime(format!(
                    "Invalid JSON from PHP worker: {} (line: {})",
                    error, trimmed
                )));
            }
        };

        match value.get("type").and_then(Value::as_str) {
            Some("host_call") => {
                handle_host_call(&value, stdin, host)?;
                continue;
            }
            Some("host_result") => {
                // Late/unsolicited result from an earlier call; ignore it.
                debug!("Ignoring unsolicited host_result from PHP worker");
                continue;
            }
            _ => {}
        }

        let response: IpcResponse = serde_json::from_value(value).map_err(|error| {
            PluginError::PhpRuntime(format!("Invalid IPC response from PHP worker: {}", error))
        })?;

        if response.status == "fatal_error" {
            let msg = response
                .message
                .unwrap_or_else(|| "PHP fatal error".to_string());
            return Err(PluginError::PhpRuntime(format!("PHP fatal error: {}", msg)));
        }

        return Ok(response);
    }
}

fn handle_host_call(
    value: &Value,
    stdin: &mut ChildStdin,
    host: &RwLock<Option<Arc<dyn PluginHost>>>,
) -> Result<(), PluginError> {
    let id = value.get("id").cloned().unwrap_or(Value::Null);
    let method = value
        .get("method")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let params = value.get("params").cloned().unwrap_or(Value::Null);

    let host = host.read().clone();
    let result = match host {
        Some(host) => host.call(method, &params),
        None => Err("No plugin host installed".to_string()),
    };

    let response = match result {
        Ok(result) => json!({
            "type": "host_result",
            "id": id,
            "ok": true,
            "result": result
        }),
        Err(error) => json!({
            "type": "host_result",
            "id": id,
            "ok": false,
            "error": error
        }),
    };

    write_json_line(stdin, &response.to_string())
}

fn write_json_line(stdin: &mut ChildStdin, line: &str) -> Result<(), PluginError> {
    stdin.write_all(line.as_bytes())?;
    stdin.write_all(b"\n")?;
    stdin.flush()?;
    Ok(())
}

/// Where a PHP plugin lives on disk.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PluginSource {
    /// A regular plugin directory containing `plugin.yml` and `src/`.
    Dir(PathBuf),
    /// A `.phar` archive accessed through PHP's `phar://` stream wrapper.
    Phar(PathBuf),
}

impl PluginSource {
    pub fn dir(path: impl Into<PathBuf>) -> Self {
        Self::Dir(path.into())
    }

    pub fn phar(path: impl Into<PathBuf>) -> Self {
        Self::Phar(path.into())
    }

    pub fn from_path(path: impl Into<PathBuf>) -> Self {
        let path = path.into();
        if path
            .extension()
            .and_then(|extension| extension.to_str())
            .map(|extension| extension.eq_ignore_ascii_case("phar"))
            .unwrap_or(false)
        {
            Self::Phar(path)
        } else {
            Self::Dir(path)
        }
    }

    pub fn path(&self) -> &Path {
        match self {
            Self::Dir(path) | Self::Phar(path) => path,
        }
    }

    pub fn is_phar(&self) -> bool {
        matches!(self, Self::Phar(_))
    }

    /// Directory used as `plugin_dir` in `load_plugin` (a `phar://` URI for
    /// archives).
    ///
    /// URI parity with PHP: PHP builds `phar://<realpath of the .phar>` (see
    /// `PharPluginLoader` in `php/plugin.php` and `readPharEntry()` in
    /// `php/ipc.php`); Rust does the same here with
    /// [`normalize_phar_path`], so both sides address identical entries.
    pub fn root(&self) -> String {
        match self {
            Self::Dir(path) => path.to_string_lossy().to_string(),
            Self::Phar(path) => {
                let path = normalize_phar_path(path);
                format!("phar://{}", path.to_string_lossy().replace('\\', "/"))
            }
        }
    }

    /// `src/` directory inside the plugin (PM `"<phar>/src"` for archives,
    /// `"<plugin dir>/src"` otherwise). The PHP autoloader resolves classes
    /// below this root via `phar://` URIs.
    pub fn src_root(&self) -> String {
        format!("{}/src", self.root().trim_end_matches('/'))
    }

    /// Absolute path of the plugin's main class file.
    pub fn main_file(&self, main_class: &str) -> String {
        let relative = main_class
            .trim()
            .trim_start_matches(|character| character == '\\' || character == '/')
            .replace('\\', "/");
        format!("{}/{}.php", self.src_root(), relative)
    }
}

/// Normalizes a `.phar` path to the filesystem path PHP's `realpath()`
/// returns: strips an optional `phar://` prefix, makes relative paths
/// absolute against the current directory, and canonicalizes when the file
/// exists (resolves symlinks and `.` / `..` segments).
fn normalize_phar_path(path: &Path) -> PathBuf {
    let raw = path.to_string_lossy();
    let stripped = raw
        .strip_prefix("phar://")
        .or_else(|| raw.strip_prefix("PHAR://"))
        .map(PathBuf::from)
        .unwrap_or_else(|| path.to_path_buf());

    let stripped = strip_windows_extended_prefix(stripped);

    let absolute = if stripped.is_absolute() {
        stripped
    } else {
        std::env::current_dir()
            .map(|cwd| cwd.join(&stripped))
            .unwrap_or(stripped)
    };

    // Canonicalize mirrors PHP `realpath()`: Windows paths become absolute
    // drive-letter paths and symlinks resolve, so `PluginSource::Phar`
    // addresses the same archive PHP opens via `phar://`.
    let canonical = std::fs::canonicalize(&absolute).unwrap_or(absolute);
    // ...but Rust's canonicalize returns the Windows extended-length form
    // (`\\?\C:\...`), which PHP's `phar://` wrapper does not understand:
    // `phar:////?/C:/...` fails to open the archive (ScoreHud main class
    // not found). PHP `realpath()` returns the plain drive-letter path.
    strip_windows_extended_prefix(canonical)
}

/// Strips the Windows extended-length prefix (`\\?\`, and its `//?/`
/// forward-slash form) that `std::fs::canonicalize` produces, restoring
/// the plain drive-letter path PHP's `realpath()` returns.
fn strip_windows_extended_prefix(path: PathBuf) -> PathBuf {
    let raw = path.to_string_lossy();
    if let Some(rest) = raw.strip_prefix(r"\\?\UNC\") {
        return PathBuf::from(format!(r"\\{rest}"));
    }
    if let Some(rest) = raw.strip_prefix(r"\\?\") {
        return PathBuf::from(rest);
    }
    if let Some(rest) = raw.strip_prefix("//?/UNC/") {
        return PathBuf::from(format!(r"\\{rest}"));
    }
    if let Some(rest) = raw.strip_prefix("//?/") {
        return PathBuf::from(rest);
    }
    path
}

/// A PHP-based PocketMine plugin managed via the IPC bridge.
pub struct PhpPlugin {
    description: PluginDescription,
    source: PluginSource,
    worker: Arc<PhpWorker>,
    enabled: AtomicBool,
    last_commands: RwLock<Vec<String>>,
    last_listener_events: RwLock<Vec<String>>,
}

impl PhpPlugin {
    pub fn new(
        description: PluginDescription,
        source: impl Into<PluginSource>,
        worker: Arc<PhpWorker>,
    ) -> Self {
        Self {
            description,
            source: source.into(),
            worker,
            enabled: AtomicBool::new(false),
            last_commands: RwLock::new(Vec::new()),
            last_listener_events: RwLock::new(Vec::new()),
        }
    }

    pub fn source(&self) -> &PluginSource {
        &self.source
    }

    pub fn last_commands(&self) -> Vec<String> {
        self.last_commands.read().clone()
    }

    pub fn last_listener_events(&self) -> Vec<String> {
        self.last_listener_events.read().clone()
    }

    fn data_folder(&self) -> PathBuf {
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join("plugin_data")
            .join(self.name())
    }
}

impl From<PathBuf> for PluginSource {
    fn from(path: PathBuf) -> Self {
        Self::from_path(path)
    }
}

impl From<&Path> for PluginSource {
    fn from(path: &Path) -> Self {
        Self::from_path(path)
    }
}

impl From<&str> for PluginSource {
    fn from(path: &str) -> Self {
        Self::from_path(PathBuf::from(path))
    }
}

impl From<String> for PluginSource {
    fn from(path: String) -> Self {
        Self::from_path(PathBuf::from(path))
    }
}

#[async_trait::async_trait]
impl Plugin for PhpPlugin {
    fn description(&self) -> &PluginDescription {
        &self.description
    }

    fn is_enabled(&self) -> bool {
        self.enabled.load(Ordering::SeqCst)
    }

    fn commands(&self) -> Vec<String> {
        self.last_commands.read().clone()
    }

    fn listener_events(&self) -> Vec<String> {
        self.last_listener_events.read().clone()
    }

    async fn on_load(&mut self) -> Result<(), PluginError> {
        let main_class = self.description.main.clone();
        let main_file = self.source.main_file(&main_class);
        let plugin_dir = self.source.root();
        let data_folder = self.data_folder();

        std::fs::create_dir_all(&data_folder)?;

        debug!(
            "Loading PHP plugin '{}' (main: {}, source: {})",
            self.name(),
            main_class,
            plugin_dir
        );

        let response = self
            .worker
            .send_command(
                "load_plugin",
                json!({
                    "name": self.name(),
                    "main": main_class,
                    "file": main_file,
                    "plugin_dir": plugin_dir,
                    "data_folder": data_folder.to_string_lossy(),
                    "description": serde_json::to_value(&self.description)?
                }),
            )
            .await?;

        if response.status == "error" {
            return Err(PluginError::PhpRuntime(format!(
                "Failed to load plugin '{}': {}",
                self.name(),
                response.message.unwrap_or_default()
            )));
        }

        if response.status != "ok" {
            warn!(
                "Unexpected status '{}' while loading plugin '{}': {:?}",
                response.status,
                self.name(),
                response.message
            );
        }

        Ok(())
    }

    async fn on_enable(&mut self) -> Result<(), PluginError> {
        debug!("Enabling PHP plugin '{}'", self.name());
        let response = self
            .worker
            .send_command("enable_plugin", json!({ "name": self.name() }))
            .await?;

        if response.status == "error" {
            return Err(PluginError::PhpRuntime(format!(
                "Failed to enable plugin '{}': {}",
                self.name(),
                response.message.unwrap_or_default()
            )));
        }

        *self.last_commands.write() = response.commands;
        *self.last_listener_events.write() = response.listener_events;
        self.enabled.store(true, Ordering::SeqCst);
        Ok(())
    }

    async fn on_disable(&mut self) -> Result<(), PluginError> {
        debug!("Disabling PHP plugin '{}'", self.name());
        let result = self
            .worker
            .send_command("disable_plugin", json!({ "name": self.name() }))
            .await;

        if let Err(error) = &result {
            warn!("Failed to disable PHP plugin '{}': {}", self.name(), error);
        }

        self.last_commands.write().clear();
        self.last_listener_events.write().clear();
        self.enabled.store(false, Ordering::SeqCst);
        result.map(|_| ())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn phar_source_builds_phar_uris() {
        let source = PluginSource::phar("plugins/Foo.phar");
        assert!(source.is_phar());
        assert!(source.root().starts_with("phar://"));
        assert!(source
            .main_file("Acme\\Foo\\Main")
            .ends_with("Foo.phar/src/Acme/Foo/Main.php"));
        assert!(source.src_root().ends_with("Foo.phar/src"));
    }

    #[test]
    fn phar_root_never_uses_windows_extended_prefix() {
        // Regression: canonicalize() on Windows yields `\\?\C:\...`, which
        // became `phar:////?/C:/...` and PHP could not open the archive.
        let source = PluginSource::phar("plugins/scorehud-8.0.1.phar");
        let root = source.root();
        assert!(root.starts_with("phar://"), "root: {root}");
        assert!(!root.contains("//?/"), "extended prefix leaked: {root}");
        assert!(!root.contains(r"\\?\"), "extended prefix leaked: {root}");
        // An already-broken extended URI input is repaired, not doubled.
        let repaired = PluginSource::phar("phar:////?/C:/plugins/Foo.phar");
        assert!(!repaired.root().contains("//?/"), "root: {}", repaired.root());
    }

    #[test]
    fn phar_source_detects_extension_case_insensitively() {
        let source = PluginSource::from_path("plugins/Bar.PHAR");
        assert!(source.is_phar());
        // An already-prefixed URI must not gain a second `phar://` prefix.
        let prefixed = PluginSource::phar("phar://plugins/Bar.phar");
        assert_eq!(prefixed.root().matches("phar://").count(), 1);
    }

    #[test]
    fn phar_src_root_matches_php_addpath() {
        // PM `PharPluginLoader::loadPlugin()` registers `"$file/src"` with the
        // class loader; Rust must address the same location.
        let source = PluginSource::from_path("plugins/MyPlugin.phar");
        let src = source.src_root();
        assert!(src.starts_with("phar://"));
        assert!(src.ends_with("MyPlugin.phar/src"));
        assert_eq!(
            source.main_file("MyPlugin\\Main"),
            format!("{src}/MyPlugin/Main.php")
        );
    }

    #[test]
    fn dir_source_builds_main_file() {
        let source = PluginSource::dir("plugins/Foo");
        assert_eq!(
            source.main_file("Acme\\Foo\\Main"),
            "plugins/Foo/src/Acme/Foo/Main.php"
        );
    }
}
