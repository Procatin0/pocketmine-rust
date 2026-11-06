<?php

declare(strict_types=1);

namespace BetterChat\command;

use pocketmine\command\CommandSender;
use pocketmine\utils\TextFormat;
use BetterChat\Main;

class BetterChatCommand{

	public function __construct(private Main $plugin){}

	public function execute(CommandSender $sender, array $args) : bool{
		if(empty($args)){
			$sender->sendMessage(TextFormat::colorize($this->plugin->msg("command.usage")));
			return true;
		}

		$subCommand = strtolower(array_shift($args));

		return match($subCommand){
			"reload" => $this->handleReload($sender),
			"mute" => $this->handleMute($sender, $args),
			"unmute" => $this->handleUnmute($sender, $args),
			"setprefix" => $this->handleSetPrefix($sender, $args),
			"setsuffix" => $this->handleSetSuffix($sender, $args),
			"minigame" => $this->handleMinigame($sender, $args),
			default => (function() use($sender) : bool{
				$sender->sendMessage(TextFormat::colorize($this->plugin->msg("command.unknown-subcommand")));
				return true;
			})(),
		};
	}

	private function handleReload(CommandSender $sender) : bool{
		if(!$sender->hasPermission("betterchat.command.reload")){
			$sender->sendMessage(TextFormat::colorize($this->plugin->msg("no-permission")));
			return true;
		}
		$this->plugin->reload();
		$sender->sendMessage(TextFormat::colorize($this->plugin->msg("command.reloaded")));
		return true;
	}

	private function handleMute(CommandSender $sender, array $args) : bool{
		if(!$sender->hasPermission("betterchat.command.mute")){
			$sender->sendMessage(TextFormat::colorize($this->plugin->msg("no-permission")));
			return true;
		}
		if(count($args) < 1){
			$sender->sendMessage(TextFormat::colorize($this->plugin->msg("command.mute-usage")));
			return true;
		}
		$targetName = array_shift($args);
		if($this->plugin->getMuteManager()->isPlayerMuted($targetName)){
			$msg = str_replace("{target}", $targetName, $this->plugin->msg("command.already-muted"));
			$sender->sendMessage(TextFormat::colorize($msg));
			return true;
		}
		// Parse duration (e.g. 30s, 5m, 2h, 1d). Default: permanent.
		$duration = 0;
		if(!empty($args)){
			$duration = $this->parseDuration(array_shift($args));
			if($duration === -1){
				$sender->sendMessage(TextFormat::colorize($this->plugin->msg("command.invalid-duration")));
				return true;
			}
		}
		// Warn if player is not online
		if($this->plugin->getServer()->getPlayerByPrefix($targetName) === null){
			$msg = str_replace("{target}", $targetName, $this->plugin->msg("command.target-offline"));
			$sender->sendMessage(TextFormat::colorize($msg));
		}
		$this->plugin->getMuteManager()->mute($targetName, $duration);
		$durationText = $duration > 0 ? " for " . $this->formatDuration($duration) : " permanently";
		$msg = str_replace(
			["{target}", "{duration}"],
			[$targetName, $durationText],
			$this->plugin->msg("command.mute-success")
		);
		$sender->sendMessage(TextFormat::colorize($msg));

		$broadcast = $this->plugin->msg("mute.broadcast-mute", "");
		if($broadcast !== ""){
			$msg = str_replace(
				["{player}", "{target}", "{duration}"],
				[$sender->getName(), $targetName, $duration > 0 ? $this->formatDuration($duration) : "permanently"],
				$broadcast
			);
			$this->plugin->getServer()->broadcastMessage(TextFormat::colorize($msg));
		}
		return true;
	}

	private function handleUnmute(CommandSender $sender, array $args) : bool{
		if(!$sender->hasPermission("betterchat.command.unmute")){
			$sender->sendMessage(TextFormat::colorize($this->plugin->msg("no-permission")));
			return true;
		}
		if(count($args) < 1){
			$sender->sendMessage(TextFormat::colorize($this->plugin->msg("command.unmute-usage")));
			return true;
		}
		$targetName = array_shift($args);
		if(!$this->plugin->getMuteManager()->isPlayerMuted($targetName)){
			$msg = str_replace("{target}", $targetName, $this->plugin->msg("command.not-muted"));
			$sender->sendMessage(TextFormat::colorize($msg));
			return true;
		}
		$this->plugin->getMuteManager()->unmute($targetName);
		$msg = str_replace("{target}", $targetName, $this->plugin->msg("command.unmute-success"));
		$sender->sendMessage(TextFormat::colorize($msg));

		$broadcast = $this->plugin->msg("mute.broadcast-unmute", "");
		if($broadcast !== ""){
			$msg = str_replace(
				["{player}", "{target}"],
				[$sender->getName(), $targetName],
				$broadcast
			);
			$this->plugin->getServer()->broadcastMessage(TextFormat::colorize($msg));
		}
		return true;
	}

	private function handleSetPrefix(CommandSender $sender, array $args) : bool{
		if(!$sender->hasPermission("betterchat.command.setprefix")){
			$sender->sendMessage(TextFormat::colorize($this->plugin->msg("no-permission")));
			return true;
		}
		if(count($args) < 2){
			$sender->sendMessage(TextFormat::colorize($this->plugin->msg("command.setprefix-usage")));
			return true;
		}
		$targetName = array_shift($args);
		$prefix = implode(" ", $args);
		$this->plugin->getMuteManager()->setCustomPrefix($targetName, $prefix);
		$msg = str_replace(
			["{target}", "{prefix}"],
			[$targetName, TextFormat::colorize($prefix)],
			$this->plugin->msg("command.prefix-set")
		);
		$sender->sendMessage(TextFormat::colorize($msg));
		return true;
	}

	private function handleSetSuffix(CommandSender $sender, array $args) : bool{
		if(!$sender->hasPermission("betterchat.command.setsuffix")){
			$sender->sendMessage(TextFormat::colorize($this->plugin->msg("no-permission")));
			return true;
		}
		if(count($args) < 2){
			$sender->sendMessage(TextFormat::colorize($this->plugin->msg("command.setsuffix-usage")));
			return true;
		}
		$targetName = array_shift($args);
		$suffix = implode(" ", $args);
		$this->plugin->getMuteManager()->setCustomSuffix($targetName, $suffix);
		$msg = str_replace(
			["{target}", "{suffix}"],
			[$targetName, TextFormat::colorize($suffix)],
			$this->plugin->msg("command.suffix-set")
		);
		$sender->sendMessage(TextFormat::colorize($msg));
		return true;
	}

	private function handleMinigame(CommandSender $sender, array $args) : bool{
		if(!$sender->hasPermission("betterchat.command.minigame")){
			$sender->sendMessage(TextFormat::colorize($this->plugin->msg("no-permission")));
			return true;
		}
		if(empty($args)){
			$sender->sendMessage(TextFormat::colorize($this->plugin->msg("command.minigame-usage")));
			return true;
		}

		$action = strtolower(array_shift($args));
		$mgMgr = $this->plugin->getMinigameManager();

		if(!$mgMgr->isEnabled()){
			$sender->sendMessage(TextFormat::colorize($this->plugin->msg("command.minigame-disabled")));
			return true;
		}

		switch($action){
			case "start":
				$type = !empty($args) ? strtolower(array_shift($args)) : null;
				if($mgMgr->hasActiveGame()){
					$sender->sendMessage(TextFormat::colorize($mgMgr->getAlreadyRunningMessage()));
					return true;
				}
				if($type === null){
					$types = $mgMgr->getEnabledTypes();
					if(empty($types)){
						$sender->sendMessage(TextFormat::colorize($mgMgr->getNoQuestionsMessage()));
						return true;
					}
					$type = $types[array_rand($types)];
				}
				if(!$mgMgr->startGame($type)){
					$sender->sendMessage(TextFormat::colorize($mgMgr->getNoQuestionsMessage()));
					return true;
				}
				return true;

			case "stop":
				if(!$mgMgr->hasActiveGame()){
					$sender->sendMessage(TextFormat::colorize($this->plugin->msg("command.minigame-not-running")));
					return true;
				}
				$mgMgr->stopGame();
				$sender->sendMessage(TextFormat::colorize($mgMgr->getStoppedMessage()));
				return true;

			default:
				$sender->sendMessage(TextFormat::colorize($this->plugin->msg("command.minigame-usage")));
				return true;
		}
	}

	/**
	 * Parse a duration string like "30s", "5m", "2h", "1d" into seconds.
	 * Returns -1 if invalid.
	 */
	private function parseDuration(string $input) : int{
		if(!preg_match('/^(\d+)([smhd])$/', strtolower($input), $matches)){
			return -1;
		}
		$value = (int) $matches[1];
		$unit = $matches[2];
		return match($unit){
			"s" => $value,
			"m" => $value * 60,
			"h" => $value * 3600,
			"d" => $value * 86400,
			default => -1,
		};
	}

	private function formatDuration(int $seconds) : string{
		if($seconds >= 86400){
			$days = intdiv($seconds, 86400);
			return $days . "d";
		}
		if($seconds >= 3600){
			$hours = intdiv($seconds, 3600);
			return $hours . "h";
		}
		if($seconds >= 60){
			$mins = intdiv($seconds, 60);
			return $mins . "m";
		}
		return $seconds . "s";
	}
}
