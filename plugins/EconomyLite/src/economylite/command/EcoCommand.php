<?php

declare(strict_types=1);

namespace economylite\command;

use economylite\Main;
use pocketmine\command\Command;
use pocketmine\command\CommandExecutor;
use pocketmine\command\CommandSender;
use pocketmine\utils\TextFormat;

/**
 * /eco give|take|set <jugador> <cantidad> (solo operadores).
 */
final class EcoCommand implements CommandExecutor{

	private const USAGE = "/eco <give|take|set> <jugador> <cantidad>";

	public function __construct(private Main $plugin){}

	public function onCommand(CommandSender $sender, Command $command, string $label, array $args) : bool{
		if(!$sender->hasPermission("economylite.command.eco")){
			$sender->sendMessage(TextFormat::RED . "No tienes permiso para administrar la economía.");
			return true;
		}

		if(count($args) !== 3){
			$sender->sendMessage(TextFormat::YELLOW . "Uso: " . self::USAGE);
			return true;
		}

		$action = strtolower($args[0]);
		if(!in_array($action, ["give", "take", "set"], true)){
			$sender->sendMessage(TextFormat::RED . "Acción desconocida. Uso: " . self::USAGE);
			return true;
		}

		if(!is_numeric($args[2])){
			$sender->sendMessage(TextFormat::RED . "La cantidad debe ser un número.");
			return true;
		}

		$amount = round((float) $args[2], 2);
		if(!is_finite($amount) || $amount < 0 || ($action !== "set" && $amount <= 0)){
			$sender->sendMessage(TextFormat::RED . "La cantidad no es válida para " . $action . ".");
			return true;
		}

		$targetPlayer = $this->plugin->getServer()->getPlayerExact($args[1]);
		$accountName = $targetPlayer?->getName() ?? $args[1];

		$newBalance = null;

		switch($action){
			case "give":
				$newBalance = $this->plugin->addBalance($accountName, $amount);
				break;

			case "take":
				$current = $this->plugin->getBalance($accountName);
				if($current === null){
					$sender->sendMessage(TextFormat::RED . "No existe una cuenta para " . $accountName . ".");
					return true;
				}
				if($current < $amount){
					$sender->sendMessage(
						TextFormat::RED . "La cuenta de " . $accountName . " solo tiene " .
						$this->plugin->formatMoney($current) . "."
					);
					return true;
				}
				$newBalance = $this->plugin->setBalance($accountName, $current - $amount);
				break;

			case "set":
				$newBalance = $this->plugin->setBalance($accountName, $amount);
				break;
		}

		$sender->sendMessage(
			TextFormat::GREEN . "EconomyLite: " . $action . " " . $this->plugin->formatMoney($amount) .
			" -> " . $accountName . " (nuevo saldo: " . $this->plugin->formatMoney($newBalance) . ")"
		);
		$this->plugin->getLogger()->info(
			"EconomyLite: " . $sender->getName() . " ejecutó " . $action . " " .
			$this->plugin->formatMoney($amount) . " sobre " . $accountName .
			" (nuevo saldo " . $this->plugin->formatMoney($newBalance) . ")"
		);

		if($targetPlayer !== null && $targetPlayer !== $sender){
			$targetPlayer->sendMessage(
				TextFormat::YELLOW . "Tu saldo ahora es " . $this->plugin->formatMoney($newBalance) . "."
			);
		}

		return true;
	}
}
