<?php

declare(strict_types=1);

namespace testkit\command;

use pocketmine\command\Command;
use pocketmine\command\CommandExecutor;
use pocketmine\command\CommandSender;
use pocketmine\player\Player;
use pocketmine\utils\TextFormat;
use testkit\Main;

/**
 * /pos: getPosition() y World.
 */
final class PosCommand implements CommandExecutor{

	public function __construct(private Main $plugin){}

	public function onCommand(CommandSender $sender, Command $command, string $label, array $args) : bool{
		if(!$sender->hasPermission("testkit.command")){
			$sender->sendMessage(TextFormat::RED . "No tienes permiso para usar /pos.");
			return true;
		}

		if(!$sender instanceof Player){
			$sender->sendMessage(TextFormat::RED . "Este comando solo se puede usar en el juego.");
			return true;
		}

		$position = $sender->getPosition();
		$world = $sender->getWorld();

		// PM5 usa getFolderName()/getDisplayName(); el contrato del runtime expone getName().
		$worldName = method_exists($world, "getName") ? $world->getName() : $world->getDisplayName();

		$sender->sendMessage(
			TextFormat::AQUA . "[TestKit] " . TextFormat::RESET .
			"posición: " . number_format($position->getX(), 2) . ", " .
			number_format($position->getY(), 2) . ", " .
			number_format($position->getZ(), 2)
		);
		$sender->sendMessage(
			TextFormat::AQUA . "[TestKit] " . TextFormat::RESET .
			"bloque: " . $position->getFloorX() . ", " . $position->getFloorY() . ", " . $position->getFloorZ() .
			" | mundo: " . $worldName
		);

		return true;
	}
}
