<?php

declare(strict_types=1);

namespace scoreboarddemo;

use pocketmine\command\Command;
use pocketmine\command\CommandExecutor;
use pocketmine\command\CommandSender;
use pocketmine\player\Player;
use pocketmine\plugin\PluginBase;
use pocketmine\scoreboard\ColorRange;
use pocketmine\scoreboard\Scoreboard;
use pocketmine\scoreboard\ScoreboardObjective;
use pocketmine\scoreboard\ScoreboardSlot;
use pocketmine\utils\TextFormat;

/**
 * Scoreboard example (Fase 7, `pocketmine\scoreboard\Scoreboard`):
 * builds one sidebar objective with scored lines, installs it with
 * `Player::setScoreboard()`, mutates it live (`addScore` re-emits the
 * full board through the change callback) and removes it with
 * `Player::removeScoreboard()`.
 *
 * Commands:
 * - `/sb` / `/sb show`: (re)show the example board.
 * - `/sb update <score> <text...>`: add/replace a line on the live board.
 * - `/sb clear`: remove the board.
 */
final class Main extends PluginBase implements CommandExecutor{

	public function onEnable() : void{
		$command = $this->getCommand("sb");
		if($command !== null){
			$command->setExecutor($this);
		}
		$this->getLogger()->info("ScoreboardDemo habilitado. Usa /sb para ver el ejemplo.");
	}

	public function onDisable() : void{
		$this->getLogger()->info("ScoreboardDemo deshabilitado.");
	}

	public static function buildDemoBoard() : Scoreboard{
		$board = new Scoreboard("Economía");
		$objective = new ScoreboardObjective("main", "Números", ScoreboardSlot::SIDEBAR, ColorRange::red());
		$objective->addScore(100, "Oro: 100");
		$objective->addScore(50, "Plata: 50");
		$objective->addScore(10, "Cobre: 10");
		$board->addObjective($objective);
		return $board;
	}

	public function onCommand(CommandSender $sender, Command $command, string $label, array $args) : bool{
		if(!$sender instanceof Player){
			$sender->sendMessage(TextFormat::RED . "Este comando solo puede usarse como jugador.");
			return true;
		}
		if(!$sender->hasPermission("scoreboarddemo.command")){
			$sender->sendMessage(TextFormat::RED . "No tienes permiso para usar /sb.");
			return true;
		}

		$sub = strtolower((string)($args[0] ?? "show"));
		if($sub === "clear"){
			$sender->removeScoreboard();
			$sender->sendMessage(TextFormat::GREEN . "Scoreboard eliminado.");
			return true;
		}
		if($sub === "update"){
			if(count($args) < 3 || !is_numeric($args[1])){
				$sender->sendMessage(TextFormat::RED . "Uso: /sb update <score> <texto...>");
				return true;
			}
			$board = $sender->getScoreboard() ?? self::buildDemoBoard();
			if($board->getObjective("main") === null){
				$board->addObjective(new ScoreboardObjective("main", "Números", ScoreboardSlot::SIDEBAR));
			}
			// Live mutation: ScoreboardObjective notifies the board,
			// which re-emits `set_scoreboard` without a new set call.
			$board->addScore("main", (int)$args[1], implode(" ", array_slice($args, 2)));
			$sender->setScoreboard($board);
			$sender->sendMessage(TextFormat::GREEN . "Línea actualizada.");
			return true;
		}

		// show (default)
		$sender->setScoreboard(self::buildDemoBoard());
		$sender->sendMessage(TextFormat::GREEN . "Scoreboard mostrado. Prueba /sb update 75 'Diamante: 75' y /sb clear.");
		return true;
	}
}
