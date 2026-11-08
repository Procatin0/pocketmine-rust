<?php

declare(strict_types=1);

namespace javabridge;

use pocketmine\plugin\PluginBase;
use pocketmine\scheduler\ClosureTask;

/**
 * Puente Minecraft Java Edition (TCP) -> servidor Bedrock.
 *
 * El plugin abre y atiende su propio socket TCP con PHP (streams no
 * bloqueantes) dentro de una tarea del scheduler. Los jugadores Java no son
 * sesiones Bedrock, pero comparten chat, mundo y nombres con Bedrock, se
 * registran en el `PlayerRegistry` virtual y aparecen en `/list`.
 */
final class Main extends PluginBase{

	private ?JavaServer $bridge = null;
	private int $viewDistance = 2;

	private int $javaGamemode = 0;
	private int $javaDifficulty = 2;
	private string $javaLevelType = "flat";
	private string $javaMotd = "PocketMine-Rust";
	private bool $bridgeEnabled = true;

	public function onEnable() : void{
		$this->saveDefaultConfig();
		$config = $this->getConfig();

		$this->bridgeEnabled = (bool)$config->get("enabled", true);
		$this->viewDistance = max(0, min(8, (int)$config->get("view-distance", 2)));
		$this->javaMotd = (string)$config->get("motd", "PocketMine-Rust");
		$this->javaGamemode = max(0, min(3, (int)$config->get("gamemode", 0)));
		$this->javaDifficulty = max(0, min(3, (int)$config->get("difficulty", 2)));
		$this->javaLevelType = (string)$config->get("level-type", "flat");

		// El puente no escucha eventos: da de alta a sus jugadores en el
		// servidor y deja que los plugins y el core hagan el resto.
		$this->getServer()->getCommandMap()->register("javabridge", new JavaBridgeCommand($this));

		if($this->bridgeEnabled){
			$this->startBridge();
		}else{
			$this->getLogger()->info("JavaBridge está desactivado en config.yml (enabled: false).");
		}

		// El puente atiende sus sockets una vez por tick del servidor.
		$this->getScheduler()->scheduleRepeatingTask(new ClosureTask(function() : void{
			if($this->bridge !== null){
				$this->bridge->tick();
			}
		}), 1);
	}

	public function onDisable() : void{
		$this->stopBridge();
		$this->getLogger()->info("JavaBridge deshabilitado.");
	}

	public function startBridge() : void{
		if($this->bridge !== null){
			return;
		}

		$config = $this->getConfig();
		$host = (string)$config->get("host", "0.0.0.0");
		$port = (int)$config->get("port", 25565);

		$server = new JavaServer($this, $host, $port, $this->viewDistance);
		try{
			$server->start();
			$this->bridge = $server;
		}catch(\Throwable $e){
			$this->getLogger()->error("No se pudo abrir el listener Java: " . $e->getMessage());
			$this->bridge = null;
		}
	}

	public function stopBridge() : void{
		if($this->bridge === null){
			return;
		}
		$this->bridge->stop();
		$this->bridge = null;
	}

	public function getJavaServer() : ?JavaServer{
		return $this->bridge;
	}

	public function isBridgeRunning() : bool{
		return $this->bridge !== null && $this->bridge->isRunning();
	}

	public function getJavaMotd() : string{
		return $this->javaMotd;
	}

	public function getJavaGamemode() : int{
		return $this->javaGamemode;
	}

	public function getJavaDifficulty() : int{
		return $this->javaDifficulty;
	}

	public function getJavaLevelType() : string{
		return $this->javaLevelType;
	}

	// ------------------------------------------------------------------
	// Chat y eventos: una sola vía, la del servidor
	// ------------------------------------------------------------------
	//
	// El puente no reenvía el chat a mano. Los jugadores Java están dados de
	// alta en el servidor, así que el chat de Bedrock y el suyo pasan por
	// `PlayerChatEvent` y el resultado se difunde con `broadcastMessage`, que
	// llega a las sesiones Bedrock y a la bandeja de los clientes Java. Un
	// plugin que formatea el chat ve las dos plataformas sin saber que existe
	// este puente.
}
