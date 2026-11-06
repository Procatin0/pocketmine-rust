<?php

declare(strict_types=1);

namespace testkit;

use pocketmine\command\CommandExecutor;
use pocketmine\event\block\BlockBreakEvent;
use pocketmine\event\Listener;
use pocketmine\event\player\PlayerInteractEvent;
use pocketmine\event\player\PlayerJoinEvent;
use pocketmine\event\player\PlayerMoveEvent;
use pocketmine\event\player\PlayerQuitEvent;
use pocketmine\plugin\PluginBase;
use pocketmine\scheduler\ClosureTask;
use pocketmine\utils\TextFormat;
use testkit\command\BlockCommand;
use testkit\command\PosCommand;
use testkit\command\TestKitCommand;
use testkit\command\TpsCommand;

/**
 * Plugin de pruebas que ejercita las partes del runtime que usan
 * los plugins reales: eventos mutables, scheduler, permisos, Config,
 * comandos y host calls de mundo/jugador.
 */
final class Main extends PluginBase implements Listener{

	/** @var array<string, int> */
	private array $movementCounts = [];

	private int $movementLogInterval = 100;

	public function onEnable() : void{
		$this->saveDefaultConfig();
		$this->movementLogInterval = max(1, (int) $this->getConfig()->get("movement-log-interval", 100));

		$this->getServer()->getPluginManager()->registerEvents($this, $this);
		$this->registerCommands();

		$this->getScheduler()->scheduleRepeatingTask(new ClosureTask(function() : void{
			$online = count($this->getServer()->getOnlinePlayers());
			$tracked = count($this->movementCounts);
			$total = array_sum($this->movementCounts);
			$this->getLogger()->info("TestKit heartbeat: {$online} jugador(es) online, {$tracked} rastreado(s), {$total} movimiento(s)");
		}), 600);

		$this->getLogger()->info("TestKit habilitado. Datos en: " . $this->getDataFolder());
	}

	public function onDisable() : void{
		$this->getLogger()->info("TestKit deshabilitado.");
	}

	private function registerCommands() : void{
		/** @var array<string, CommandExecutor> $executors */
		$executors = [
			"testkit" => new TestKitCommand($this),
			"pos" => new PosCommand($this),
			"block" => new BlockCommand($this),
			"tps" => new TpsCommand($this)
		];

		foreach($executors as $name => $executor){
			$command = $this->getCommand($name);
			if($command === null){
				$this->getLogger()->warning("No se encontró el comando /{$name} en plugin.yml");
				continue;
			}

			$command->setExecutor($executor);
		}
	}

	public function onJoin(PlayerJoinEvent $event) : void{
		$player = $event->getPlayer();
		$name = $player->getName();

		if(!isset($this->movementCounts[$name])){
			$this->movementCounts[$name] = 0;
		}

		$this->getLogger()->info("TestKit: entró " . $name);
		$event->setJoinMessage(TextFormat::GREEN . "[+] " . $name);
	}

	public function onQuit(PlayerQuitEvent $event) : void{
		$player = $event->getPlayer();
		$name = $player->getName();
		$moves = $this->movementCounts[$name] ?? 0;

		$this->getLogger()->info("TestKit: salió " . $name . " con " . $moves . " movimiento(s) registrados");
		$event->setQuitMessage(TextFormat::RED . "[-] " . $name);

		unset($this->movementCounts[$name]);
	}

	/**
	 * Cuenta movimientos sin modificar el evento: MONITOR es solo observación.
	 *
	 * @priority MONITOR
	 * @ignoreCancelled true
	 */
	public function onMove(PlayerMoveEvent $event) : void{
		$player = $event->getPlayer();
		$name = $player->getName();

		$count = ($this->movementCounts[$name] ?? 0) + 1;
		$this->movementCounts[$name] = $count;

		if($count % $this->movementLogInterval !== 0){
			return;
		}

		$to = $event->getTo();
		$this->getLogger()->info(
			"TestKit: " . $name . " lleva " . $count . " movimientos; último en " .
			$to->getFloorX() . ", " . $to->getFloorY() . ", " . $to->getFloorZ()
		);
	}

	/**
	 * Observa roturas de bloque (el servidor las aplica al mundo salvo
	 * cancelación): MONITOR es solo observación.
	 *
	 * @priority MONITOR
	 * @ignoreCancelled true
	 */
	public function onBlockBreak(BlockBreakEvent $event) : void{
		$this->getLogger()->info(
		"TestKit: " . $event->getPlayer()->getName() . " rompió bloque en " .
		$event->getBlock()->getX() . ", " . $event->getBlock()->getY() . ", " . $event->getBlock()->getZ()
		);
	}

	/**
	 * Observa inicios de rotura (clic izquierdo): MONITOR es solo observación.
	 *
	 * @priority MONITOR
	 * @ignoreCancelled true
	 */
	public function onInteract(PlayerInteractEvent $event) : void{
		if($event->getAction() !== PlayerInteractEvent::ACTION_LEFT_CLICK){
			return;
		}
		$this->getLogger()->info(
		"TestKit: " . $event->getPlayer()->getName() . " inició rotura en " .
		$event->getBlock()->getX() . ", " . $event->getBlock()->getY() . ", " . $event->getBlock()->getZ()
		);
	}

	public function getMovementLogInterval() : int{
		return $this->movementLogInterval;
	}

	public function getMovementCount(string $playerName) : int{
		return $this->movementCounts[$playerName] ?? 0;
	}

	public function getTrackedPlayerCount() : int{
		return count($this->movementCounts);
	}

	public function getTotalMoveCount() : int{
		return array_sum($this->movementCounts);
	}

	/**
	 * @return string[]
	 */
	public function getPluginApiVersions() : array{
		$description = $this->getDescription();

		// PM5 expone getCompatibleApis(); getApi() es el nombre histórico de PM4.
		if(method_exists($description, "getCompatibleApis")){
			$api = $description->getCompatibleApis();
			return is_array($api) ? array_map("strval", $api) : [];
		}
		if(method_exists($description, "getApi")){
			$api = $description->getApi();
			return is_array($api) ? array_map("strval", $api) : [];
		}

		return [];
	}

	public function getExamplePluginStatus() : string{
		$plugin = $this->getServer()->getPluginManager()->getPlugin("ExamplePlugin");
		if($plugin === null){
			return "no instalado";
		}
		if(!$plugin->isEnabled()){
			return "instalado pero deshabilitado";
		}

		return "habilitado v" . $plugin->getDescription()->getVersion();
	}
}
