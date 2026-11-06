<?php

declare(strict_types=1);

namespace testkit\command;

use pocketmine\command\Command;
use pocketmine\command\CommandExecutor;
use pocketmine\command\CommandSender;
use pocketmine\utils\TextFormat;
use testkit\Main;

/**
 * /tps: getTicksPerSecond() del servidor.
 */
final class TpsCommand implements CommandExecutor{

	public function __construct(private Main $plugin){}

	public function onCommand(CommandSender $sender, Command $command, string $label, array $args) : bool{
		if(!$sender->hasPermission("testkit.command")){
			$sender->sendMessage(TextFormat::RED . "No tienes permiso para usar /tps.");
			return true;
		}

		$tps = $this->plugin->getServer()->getTicksPerSecond();
		$sender->sendMessage(
			TextFormat::AQUA . "[TestKit] " . TextFormat::RESET .
			"TPS: " . TextFormat::GREEN . number_format($tps, 2) .
			TextFormat::RESET . " | tick: " . $this->plugin->getServer()->getTick()
		);

		return true;
	}
}
