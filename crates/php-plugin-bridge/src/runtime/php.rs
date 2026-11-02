use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, Lines};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};
use tokio::sync::Mutex;
use tracing::{debug, info, warn};

use crate::context::PluginAction;
use crate::description::PluginDescription;
use crate::error::PluginError;
use crate::event::Event;
use crate::plugin::Plugin;

/// Embedded PHP script defining PocketMine-MP API stubs and the JSON IPC loop.
pub const POCKETMINE_BOOTSTRAP_PHP: &str = r#"<?php
declare(strict_types=1);

namespace pocketmine {
    class Server {
        private static ?Server $instance = null;
        public array $actions = [];

        public static function getInstance(): Server {
            if (self::$instance === null) {
                self::$instance = new Server();
            }
            return self::$instance;
        }

        public function broadcastMessage(string $message): void {
            $this->actions[] = [
                'action' => 'broadcast_message',
                'data' => ['message' => $message]
            ];
        }

        public function getLogger(): Logger {
            return new Logger();
        }
    }

    class Logger {
        public function info(string $msg): void {
            fwrite(STDERR, "[PHP/INFO] " . $msg . PHP_EOL);
        }
        public function warning(string $msg): void {
            fwrite(STDERR, "[PHP/WARN] " . $msg . PHP_EOL);
        }
        public function error(string $msg): void {
            fwrite(STDERR, "[PHP/ERROR] " . $msg . PHP_EOL);
        }
        public function debug(string $msg): void {
            fwrite(STDERR, "[PHP/DEBUG] " . $msg . PHP_EOL);
        }
    }
}

namespace pocketmine\player {
    use pocketmine\Server;

    class Player {
        public string $name;
        public function __construct(string $name) {
            $this->name = $name;
        }
        public function getName(): string {
            return $this->name;
        }
        public function sendMessage(string $message): void {
            Server::getInstance()->actions[] = [
                'action' => 'send_message',
                'data' => [
                    'target_player' => $this->name,
                    'message' => $message
                ]
            ];
        }
        public function kick(string $reason = ""): void {
            Server::getInstance()->actions[] = [
                'action' => 'kick_player',
                'data' => [
                    'target_player' => $this->name,
                    'reason' => $reason
                ]
            ];
        }
    }
}

namespace pocketmine\event {
    abstract class Event {
        private bool $isCancelled = false;
        public function isCancelled(): bool {
            return $this->isCancelled;
        }
        public function cancel(): void {
            $this->isCancelled = true;
        }
    }
    interface Listener {}
    interface Cancellable {}
}

namespace pocketmine\event\player {
    use pocketmine\event\Event;
    use pocketmine\event\Cancellable;
    use pocketmine\player\Player;

    class PlayerJoinEvent extends Event {
        public Player $player;
        public string $joinMessage;
        public function __construct(Player $player, string $joinMessage) {
            $this->player = $player;
            $this->joinMessage = $joinMessage;
        }
        public function getPlayer(): Player { return $this->player; }
        public function getJoinMessage(): string { return $this->joinMessage; }
        public function setJoinMessage(string $msg): void { $this->joinMessage = $msg; }
    }

    class PlayerQuitEvent extends Event {
        public Player $player;
        public string $quitMessage;
        public function __construct(Player $player, string $quitMessage) {
            $this->player = $player;
            $this->quitMessage = $quitMessage;
        }
        public function getPlayer(): Player { return $this->player; }
        public function getQuitMessage(): string { return $this->quitMessage; }
    }

    class PlayerChatEvent extends Event implements Cancellable {
        public Player $player;
        public string $message;
        public function __construct(Player $player, string $message) {
            $this->player = $player;
            $this->message = $message;
        }
        public function getPlayer(): Player { return $this->player; }
        public function getMessage(): string { return $this->message; }
        public function setMessage(string $msg): void { $this->message = $msg; }
    }
}

namespace pocketmine\plugin {
    use pocketmine\Server;
    use pocketmine\Logger;

    abstract class PluginBase {
        public string $name = "";
        public function onLoad(): void {}
        public function onEnable(): void {}
        public function onDisable(): void {}
        public function getServer(): Server { return Server::getInstance(); }
        public function getLogger(): Logger { return Server::getInstance()->getLogger(); }
    }
}

// IPC Protocol Loop
namespace {
    use pocketmine\Server;
    use pocketmine\player\Player;
    use pocketmine\event\player\PlayerJoinEvent;
    use pocketmine\event\player\PlayerQuitEvent;
    use pocketmine\event\player\PlayerChatEvent;

    $plugins = [];
    $listeners = []; // [eventName => [callable, ...]]

    while ($line = fgets(STDIN)) {
        $req = json_decode(trim($line), true);
        if (!$req || !isset($req['action'])) {
            continue;
        }

        $server = Server::getInstance();
        $server->actions = [];
        $resp = ['status' => 'ok', 'cancelled' => false, 'actions' => []];

        try {
            switch ($req['action']) {
                case 'ping':
                    $resp['data'] = 'pong';
                    break;

                case 'load_plugin':
                    $file = $req['file'];
                    $mainClass = $req['main'];
                    if (file_exists($file)) {
                        require_once $file;
                    }
                    if (class_exists($mainClass)) {
                        $plugin = new $mainClass();
                        $plugin->name = $req['name'];
                        $plugin->onLoad();
                        $plugins[$req['name']] = $plugin;
                        $resp['message'] = "Plugin {$req['name']} loaded";
                    } else {
                        $resp['status'] = 'error';
                        $resp['message'] = "Main class {$mainClass} not found";
                    }
                    break;

                case 'enable_plugin':
                    $name = $req['name'];
                    if (isset($plugins[$name])) {
                        $plugins[$name]->onEnable();
                        // Scan for methods with Listener or standard events
                        $ref = new ReflectionClass($plugins[$name]);
                        foreach ($ref->getMethods() as $method) {
                            $params = $method->getParameters();
                            if (count($params) === 1) {
                                $type = $params[0]->getType();
                                if ($type && !$type->isBuiltin()) {
                                    $className = $type->getName();
                                    $shortName = substr($className, strrpos($className, '\\') + 1);
                                    $listeners[$shortName][] = [$plugins[$name], $method->getName()];
                                }
                            }
                        }
                    }
                    break;

                case 'dispatch_event':
                    $eventName = $req['event'];
                    $data = $req['data'] ?? [];
                    $eventObj = null;

                    if ($eventName === 'PlayerJoinEvent') {
                        $p = new Player($data['player']['name'] ?? 'Steve');
                        $eventObj = new PlayerJoinEvent($p, $data['join_message'] ?? '');
                    } elseif ($eventName === 'PlayerQuitEvent') {
                        $p = new Player($data['player']['name'] ?? 'Steve');
                        $eventObj = new PlayerQuitEvent($p, $data['quit_message'] ?? '');
                    } elseif ($eventName === 'PlayerChatEvent') {
                        $p = new Player($data['player']['name'] ?? 'Steve');
                        $eventObj = new PlayerChatEvent($p, $data['message'] ?? '');
                    }

                    if ($eventObj && isset($listeners[$eventName])) {
                        foreach ($listeners[$eventName] as $callable) {
                            $callable($eventObj);
                        }
                        $resp['cancelled'] = $eventObj->isCancelled();
                    }
                    break;

                case 'disable_plugin':
                    $name = $req['name'];
                    if (isset($plugins[$name])) {
                        $plugins[$name]->onDisable();
                        unset($plugins[$name]);
                    }
                    break;
            }
        } catch (\Throwable $e) {
            $resp['status'] = 'error';
            $resp['message'] = $e->getMessage();
        }

        $resp['actions'] = $server->actions;
        echo json_encode($resp) . PHP_EOL;
        fflush(STDOUT);
    }
}
"#;

#[allow(dead_code)]
#[derive(Debug, Serialize, Deserialize)]
struct IpcRequest {
    action: String,
    #[serde(flatten)]
    extra: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct IpcResponse {
    pub status: String,
    #[serde(default)]
    pub message: Option<String>,
    #[serde(default)]
    pub cancelled: bool,
    #[serde(default)]
    pub actions: Vec<PluginAction>,
}

struct WorkerStream {
    _child: Child,
    stdin: ChildStdin,
    lines: Lines<BufReader<ChildStdout>>,
}

/// The IPC Bridge managing the PHP worker subprocess.
pub struct PhpWorker {
    stream: Mutex<Option<WorkerStream>>,
    is_available: AtomicBool,
}

impl Default for PhpWorker {
    fn default() -> Self {
        Self::new()
    }
}

impl PhpWorker {
    pub fn new() -> Self {
        Self {
            stream: Mutex::new(None),
            is_available: AtomicBool::new(false),
        }
    }

    pub fn is_available(&self) -> bool {
        self.is_available.load(Ordering::SeqCst)
    }

    /// Attempts to spawn the PHP worker process.
    pub async fn start(&self) -> Result<bool, PluginError> {
        let mut child = match Command::new("php")
            .arg("-r")
            .arg(POCKETMINE_BOOTSTRAP_PHP)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
        {
            Ok(c) => c,
            Err(e) => {
                debug!("PHP executable not available on this host: {}", e);
                self.is_available.store(false, Ordering::SeqCst);
                return Ok(false);
            }
        };

        let stdin = child.stdin.take().ok_or_else(|| {
            PluginError::PhpRuntime("Failed to capture child stdin".to_string())
        })?;
        let stdout = child.stdout.take().ok_or_else(|| {
            PluginError::PhpRuntime("Failed to capture child stdout".to_string())
        })?;
        let reader = BufReader::new(stdout);
        let lines = reader.lines();

        let mut lock = self.stream.lock().await;
        *lock = Some(WorkerStream {
            _child: child,
            stdin,
            lines,
        });

        self.is_available.store(true, Ordering::SeqCst);
        info!("PHP plugin runtime worker started successfully");
        Ok(true)
    }

    /// Sends a JSON IPC command to the PHP worker and waits for the response.
    pub async fn send_command(&self, action: &str, extra: serde_json::Value) -> Result<IpcResponse, PluginError> {
        let mut lock = self.stream.lock().await;
        let worker = match lock.as_mut() {
            Some(w) => w,
            None => {
                return Ok(IpcResponse {
                    status: "offline".to_string(),
                    message: Some("PHP worker offline".to_string()),
                    cancelled: false,
                    actions: Vec::new(),
                });
            }
        };

        let mut payload = serde_json::json!({ "action": action });
        if let serde_json::Value::Object(map) = extra {
            for (k, v) in map {
                payload[k] = v;
            }
        }

        let json_str = serde_json::to_string(&payload)? + "\n";
        worker.stdin.write_all(json_str.as_bytes()).await?;
        worker.stdin.flush().await?;

        if let Some(line) = worker.lines.next_line().await? {
            let resp: IpcResponse = serde_json::from_str(&line)?;
            Ok(resp)
        } else {
            Err(PluginError::PhpRuntime("Worker closed stdout stream unexpectedly".to_string()))
        }
    }

    /// Dispatches an event to the PHP worker.
    pub async fn dispatch_event(&self, event: &mut dyn Event) -> Result<Vec<PluginAction>, PluginError> {
        if !self.is_available() {
            return Ok(Vec::new());
        }

        let resp = self
            .send_command(
                "dispatch_event",
                serde_json::json!({
                    "event": event.name(),
                    "data": event.to_json()
                }),
            )
            .await?;

        if resp.cancelled && event.is_cancellable() {
            event.set_cancelled(true);
        }

        Ok(resp.actions)
    }
}

/// A PHP-based PocketMine plugin managed via the IPC bridge.
pub struct PhpPlugin {
    description: PluginDescription,
    plugin_dir: PathBuf,
    worker: Arc<PhpWorker>,
    enabled: AtomicBool,
}

impl PhpPlugin {
    pub fn new(description: PluginDescription, plugin_dir: PathBuf, worker: Arc<PhpWorker>) -> Self {
        Self {
            description,
            plugin_dir,
            worker,
            enabled: AtomicBool::new(false),
        }
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

    async fn on_load(&mut self) -> Result<(), PluginError> {
        let main_class = &self.description.main;
        let main_file = self.plugin_dir.join("src").join(
            main_class.replace('\\', "/").trim_start_matches('/')
        ).with_extension("php");

        info!("Loading PHP plugin '{}' (main: {})", self.name(), main_class);
        let resp = self.worker.send_command("load_plugin", serde_json::json!({
            "name": self.name(),
            "file": main_file.to_string_lossy(),
            "main": main_class
        })).await?;

        if resp.status != "ok" && resp.status != "offline" {
            warn!("Failed to load PHP plugin {}: {:?}", self.name(), resp.message);
        }

        Ok(())
    }

    async fn on_enable(&mut self) -> Result<(), PluginError> {
        info!("Enabling PHP plugin '{}'", self.name());
        let resp = self.worker.send_command("enable_plugin", serde_json::json!({
            "name": self.name()
        })).await?;

        if resp.status == "ok" || resp.status == "offline" {
            self.enabled.store(true, Ordering::SeqCst);
        }
        Ok(())
    }

    async fn on_disable(&mut self) -> Result<(), PluginError> {
        info!("Disabling PHP plugin '{}'", self.name());
        let _ = self.worker.send_command("disable_plugin", serde_json::json!({
            "name": self.name()
        })).await;
        self.enabled.store(false, Ordering::SeqCst);
        Ok(())
    }
}
