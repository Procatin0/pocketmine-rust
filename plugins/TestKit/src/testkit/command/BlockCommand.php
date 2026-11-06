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
 * /block: lectura de un bloque mediante el host call world.get_block.
 * No modifica el mundo.
 */
final class BlockCommand implements CommandExecutor{

	public function __construct(private Main $plugin){}

	public function onCommand(CommandSender $sender, Command $command, string $label, array $args) : bool{
		if(!$sender->hasPermission("testkit.command")){
			$sender->sendMessage(TextFormat::RED . "No tienes permiso para usar /block.");
			return true;
		}

		if(!$sender instanceof Player){
			$sender->sendMessage(TextFormat::RED . "Este comando solo se puede usar en el juego.");
			return true;
		}

		$position = $sender->getPosition()->add(0, -1, 0);
		$block = $sender->getWorld()->getBlock($position);

		$line = "bloque bajo el jugador: " . $block->getName() .
			" (typeId=" . $block->getTypeId();

		// getId() era la API numérica de PM4; PM5 usa type IDs y states.
		// Se informa solo si el runtime conserva el alias.
		if(method_exists($block, "getId")){
			$line .= ", legacy getId=" . $block->getId();
		}
		$line .= ")";

		$sender->sendMessage(TextFormat::AQUA . "[TestKit] " . TextFormat::RESET . $line);
		$sender->sendMessage(
			TextFormat::GRAY . "coordenadas: " .
			$position->getFloorX() . ", " . $position->getFloorY() . ", " . $position->getFloorZ()
		);

		return true;
	}
}
