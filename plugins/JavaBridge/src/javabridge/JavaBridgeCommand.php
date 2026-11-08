<?php

declare(strict_types=1);

namespace javabridge;

use pocketmine\command\Command;
use pocketmine\command\CommandSender;
use pocketmine\utils\TextFormat;

/**
 * /javabridge start|stop|status|reload
 */
final class JavaBridgeCommand extends Command{

	public function __construct(private Main $plugin){
		parent::__construct(
			"javabridge",
			"Estado y control del puente Minecraft Java -> Bedrock.",
			"/javabridge <start|stop|status|reload>",
			["jb"]
		);

		$this->setPermission("javabridge.command");
		$this->setPermissionMessage(TextFormat::RED . "No tienes permiso para administrar JavaBridge.");
	}

	public function execute(CommandSender $sender, string $commandLabel, array $args) : bool{
		if(!$sender->hasPermission("javabridge.command")){
			$sender->sendMessage(TextFormat::RED . "No tienes permiso para administrar JavaBridge.");
			return true;
		}

		$action = strtolower($args[0] ?? "status");

		switch($action){
			case "start":
				$this->plugin->startBridge();
				$sender->sendMessage($this->plugin->isBridgeRunning()
					? TextFormat::GREEN . "JavaBridge: listener Java abierto."
					: TextFormat::RED . "JavaBridge: no se pudo abrir el listener; revisa la consola.");
				break;

			case "stop":
				$this->plugin->stopBridge();
				$sender->sendMessage(TextFormat::YELLOW . "JavaBridge: listener Java cerrado.");
				break;

			case "reload":
				$this->plugin->stopBridge();
				$this->plugin->reloadConfig();
				$this->plugin->startBridge();
				$sender->sendMessage(TextFormat::YELLOW . "JavaBridge: configuración recargada.");
				break;

			case "status":
				$server = $this->plugin->getJavaServer();
				$state = $this->plugin->isBridgeRunning()
					? TextFormat::GREEN . "escuchando"
					: TextFormat::RED . "parado";

				$sender->sendMessage(TextFormat::AQUA . "[JavaBridge] " . TextFormat::RESET . $state);
				if($server !== null){
					$sender->sendMessage(TextFormat::GRAY . "Endpoint: tcp://" . $server->getHost() . ":" . $server->getPort());
					$sender->sendMessage(TextFormat::GRAY . "Clientes Java conectados: " . count($server->javaPlayerNames()));
					$names = $server->javaPlayerNames();
					if($names !== []){
						$sender->sendMessage(TextFormat::GRAY . "Jugadores: " . implode(", ", $names));
					}
					$sender->sendMessage(TextFormat::GRAY . "Radios de chunks: " . $server->viewDistance());
				}
				$sender->sendMessage(TextFormat::GRAY . "Versiones soportadas: " . Protocol::VERSION_NAME . " (protocolo " . Protocol::PROTOCOL_VERSION . ") y " . implode(', ', ModernProfile::VERSION_NAMES) . ".");
				$sender->sendMessage(TextFormat::GRAY . "MOTD Java: " . $this->plugin->getJavaMotd());
				break;

			default:
				$sender->sendMessage(TextFormat::YELLOW . "Uso: /javabridge <start|stop|status|reload>");
				break;
		}

		return true;
	}
}
