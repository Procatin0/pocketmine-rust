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
 * /pay <jugador> <cantidad>.
 */
final class PayCommand implements CommandExecutor{

	public function __construct(private Main $plugin){}

	public function onCommand(CommandSender $sender, Command $command, string $label, array $args) : bool{
		if(!$sender->hasPermission("economylite.command.pay")){
			$sender->sendMessage(TextFormat::RED . "No tienes permiso para usar /pay.");
			return true;
		}

		if(!$sender instanceof Player){
			$sender->sendMessage(TextFormat::RED . "Este comando solo se puede usar en el juego.");
			return true;
		}

		if(count($args) !== 2){
			$sender->sendMessage(TextFormat::YELLOW . "Uso: /pay <jugador> <cantidad>");
			return true;
		}

		if(!is_numeric($args[1])){
			$sender->sendMessage(TextFormat::RED . "La cantidad debe ser un número.");
			return true;
		}

		$amount = round((float) $args[1], 2);
		if(!is_finite($amount) || $amount <= 0){
			$sender->sendMessage(TextFormat::RED . "La cantidad debe ser mayor que cero.");
			return true;
		}

		$target = $this->plugin->getServer()->getPlayerExact($args[0]);
		if($target === null){
			$sender->sendMessage(TextFormat::RED . "No se encontró un jugador online llamado " . $args[0] . ".");
			$names = $this->plugin->getOnlinePlayerNames();
			$sender->sendMessage(TextFormat::GRAY . "Jugadores online: " . (count($names) > 0 ? implode(", ", $names) : "ninguno"));
			return true;
		}

		if(strcasecmp($target->getName(), $sender->getName()) === 0){
			$sender->sendMessage(TextFormat::RED . "No puedes pagarte a ti mismo.");
			return true;
		}

		$balance = $this->plugin->getOrCreateBalance($sender->getName());
		if($balance < $amount){
			$sender->sendMessage(
				TextFormat::RED . "Fondos insuficientes. Tu saldo es " .
				$this->plugin->formatMoney($balance) . "."
			);
			return true;
		}

		if(!$this->plugin->transfer($sender->getName(), $target->getName(), $amount)){
			$sender->sendMessage(TextFormat::RED . "No se pudo completar la transferencia.");
			return true;
		}

		$senderBalance = $this->plugin->getBalance($sender->getName()) ?? 0.0;
		$targetBalance = $this->plugin->getBalance($target->getName()) ?? 0.0;

		$sender->sendMessage(
			TextFormat::GREEN . "Enviaste " . TextFormat::YELLOW . $this->plugin->formatMoney($amount) .
			TextFormat::GREEN . " a " . $target->getName() . ". Saldo restante: " .
			$this->plugin->formatMoney($senderBalance)
		);
		$target->sendMessage(
			TextFormat::GREEN . "Recibiste " . TextFormat::YELLOW . $this->plugin->formatMoney($amount) .
			TextFormat::GREEN . " de " . $sender->getName() . ". Saldo actual: " .
			$this->plugin->formatMoney($targetBalance)
		);

		return true;
	}
}
