<?php

declare(strict_types=1);

namespace testkit\command;

use pocketmine\command\Command;
use pocketmine\command\CommandExecutor;
use pocketmine\command\CommandSender;
use pocketmine\utils\TextFormat;
use testkit\Main;

/**
 * /testkit y /tk: estado del runtime y del servidor.
 */
final class TestKitCommand implements CommandExecutor{

	public function __construct(private Main $plugin){}

	public function onCommand(CommandSender $sender, Command $command, string $label, array $args) : bool{
		if(!$sender->hasPermission("testkit.command")){
			$sender->sendMessage(TextFormat::RED . "No tienes permiso para usar /testkit.");
			return true;
		}

		$apiVersions = $this->plugin->getPluginApiVersions();
		$lines = [
			"versión del plugin: " . $this->plugin->getDescription()->getVersion(),
			"API declarada: " . (count($apiVersions) > 0 ? implode(", ", $apiVersions) : "desconocida"),
			"tick actual: " . $this->plugin->getServer()->getTick(),
			"jugadores online: " . count($this->plugin->getServer()->getOnlinePlayers()),
			"ExamplePlugin: " . $this->plugin->getExamplePluginStatus(),
			"data folder: " . $this->plugin->getDataFolder(),
			"movement-log-interval: " . $this->plugin->getMovementLogInterval(),
			"movimientos totales: " . $this->plugin->getTotalMoveCount()
		];

		foreach($lines as $line){
			$sender->sendMessage(TextFormat::AQUA . "[TestKit] " . TextFormat::RESET . $line);
		}

		return true;
	}
}
