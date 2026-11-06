<?php

declare(strict_types=1);

namespace economylite\command;

use economylite\Main;
use pocketmine\command\Command;
use pocketmine\command\CommandExecutor;
use pocketmine\command\CommandSender;
use pocketmine\player\Player;
use pocketmine\utils\TextFormat;

/**
 * /money [jugador] (alias /bal y /balance).
 */
final class MoneyCommand implements CommandExecutor{

	public function __construct(private Main $plugin){}

	public function onCommand(CommandSender $sender, Command $command, string $label, array $args) : bool{
		if(!$sender->hasPermission("economylite.command.money")){
			$sender->sendMessage(TextFormat::RED . "No tienes permiso para consultar saldos.");
			return true;
		}

		$isSelf = false;

		if(count($args) === 0){
			if(!$sender instanceof Player){
				$sender->sendMessage(TextFormat::RED . "Uso: /money <jugador>");
				return true;
			}

			$targetName = $sender->getName();
			$isSelf = true;
		}else{
			$targetName = $args[0];
			$online = $this->plugin->getServer()->getPlayerExact($targetName);

			if($online !== null){
				$targetName = $online->getName();
			}else{
				$knownAccount = $this->plugin->findAccountName($targetName);
				if($knownAccount === null){
					$sender->sendMessage(TextFormat::RED . "No existe una cuenta para " . $targetName . ".");
					$this->sendOnlinePlayers($sender);
					return true;
				}
				$targetName = $knownAccount;
			}

			if($sender instanceof Player){
				$isSelf = strcasecmp($targetName, $sender->getName()) === 0;
			}

			if(!$isSelf && !$sender->hasPermission("economylite.command.money.other")){
				$sender->sendMessage(TextFormat::RED . "No tienes permiso para ver el saldo de otros jugadores.");
				return true;
			}
		}

		$balance = $isSelf
			? $this->plugin->getOrCreateBalance($targetName)
			: ($this->plugin->getBalance($targetName) ?? $this->plugin->getOrCreateBalance($targetName));

		$prefix = $isSelf ? "Tu saldo" : "Saldo de " . $targetName;
		$sender->sendMessage(TextFormat::GREEN . $prefix . ": " . TextFormat::YELLOW . $this->plugin->formatMoney($balance));

		return true;
	}

	private function sendOnlinePlayers(CommandSender $sender) : void{
		$names = $this->plugin->getOnlinePlayerNames();
		$sender->sendMessage(
			TextFormat::GRAY . "Jugadores online: " .
			(count($names) > 0 ? implode(", ", $names) : "ninguno")
		);
	}
}
