<?php

declare(strict_types=1);

namespace chatguard;

use pocketmine\command\Command;
use pocketmine\command\CommandSender;
use pocketmine\utils\TextFormat;

/**
 * Comando /chatguard registrado con CommandMap.
 *
 * Al no venir de plugin.yml, el permiso debe asignarse explícitamente
 * con setPermission() antes de registrar el comando.
 */
final class ChatGuardCommand extends Command{

	public function __construct(private Main $plugin){
		parent::__construct(
			"chatguard",
			"Muestra o cambia el estado del filtro de chat.",
			"/chatguard <on|off|status>",
			["cg"]
		);

		$this->setPermission("chatguard.command");
		$this->setPermissionMessage(TextFormat::RED . "No tienes permiso para administrar ChatGuard.");
	}

	public function execute(CommandSender $sender, string $commandLabel, array $args) : bool{
		if(!$sender->hasPermission("chatguard.command")){
			$sender->sendMessage(TextFormat::RED . "No tienes permiso para administrar ChatGuard.");
			return true;
		}

		$action = strtolower($args[0] ?? "status");

		switch($action){
			case "on":
				$this->plugin->setFilterEnabled(true);
				$sender->sendMessage(TextFormat::GREEN . "ChatGuard: filtro activado.");
				$this->plugin->getLogger()->info("Filtro activado por " . $sender->getName());
				break;

			case "off":
				$this->plugin->setFilterEnabled(false);
				$sender->sendMessage(TextFormat::YELLOW . "ChatGuard: filtro desactivado.");
				$this->plugin->getLogger()->info("Filtro desactivado por " . $sender->getName());
				break;

			case "status":
				$state = $this->plugin->isFilterEnabled() ? TextFormat::GREEN . "activo" : TextFormat::RED . "inactivo";
				$sender->sendMessage(TextFormat::AQUA . "ChatGuard: " . TextFormat::RESET . $state);
				$sender->sendMessage(TextFormat::GRAY . "Palabras filtradas: " . implode(", ", $this->plugin->getBlockedWords()));
				$sender->sendMessage(TextFormat::GRAY . "Uso: /chatguard <on|off|status>");
				break;

			default:
				$sender->sendMessage(TextFormat::YELLOW . "Uso: /chatguard <on|off|status>");
				break;
		}

		return true;
	}
}
