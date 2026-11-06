<?php

declare(strict_types=1);

namespace BetterChat\manager;

use pocketmine\player\Player;
use pocketmine\plugin\PluginBase;
use pocketmine\scheduler\ClosureTask;
use pocketmine\scheduler\TaskHandler;
use pocketmine\utils\Config;
use pocketmine\utils\TextFormat;

class VoteMuteManager{

	private bool $enabled;
	private int $minOnline;
	private int $voteDuration;
	private float $yesPercentage;
	private int $muteDuration;
	private int $cooldown;
	private string $bypassPermission;

	private ?string $targetName = null;
	private string $initiatorName = "";
	/** @var array<string, bool> player name => true (yes) / false (no) */
	private array $votes = [];
	private ?TaskHandler $timerHandler = null;
	/** @var array<string, int> player name => last vote initiate timestamp */
	private array $cooldowns = [];

	public function __construct(
		private PluginBase $plugin,
		Config $config,
		private Config $messages
	){
		$p = "vote-mute.";
		$this->enabled = $config->getNested($p . "enabled", true);
		$this->minOnline = $config->getNested($p . "min-online-players", 4);
		$this->voteDuration = $config->getNested($p . "vote-duration", 30);
		$this->yesPercentage = $config->getNested($p . "yes-percentage", 60) / 100;
		$this->muteDuration = $config->getNested($p . "mute-duration", 300);
		$this->cooldown = $config->getNested($p . "cooldown", 60);
		$this->bypassPermission = $config->getNested($p . "bypass-permission", "betterchat.votemute.bypass");
	}

	public function isEnabled() : bool{
		return $this->enabled;
	}

	public function isVoteActive() : bool{
		return $this->targetName !== null;
	}

	public function startVote(Player $initiator, string $targetName) : ?string{
		if($this->isVoteActive()){
			return $this->msg("already-active");
		}

		$lname = strtolower($initiator->getName());
		if(isset($this->cooldowns[$lname]) && time() - $this->cooldowns[$lname] < $this->cooldown){
			$remaining = $this->cooldown - (time() - $this->cooldowns[$lname]);
			return $this->msg("cooldown", ["{time}" => (string) $remaining]);
		}

		$onlineCount = count($this->plugin->getServer()->getOnlinePlayers());
		if($onlineCount < $this->minOnline){
			return $this->msg("not-enough-players", ["{min}" => (string) $this->minOnline]);
		}

		$target = $this->plugin->getServer()->getPlayerByPrefix($targetName);
		if($target === null){
			return $this->msg("target-offline", ["{target}" => $targetName]);
		}

		$ltarget = strtolower($target->getName());
		if($ltarget === strtolower($initiator->getName())){
			return $this->msg("cannot-self");
		}

		if($target->hasPermission($this->bypassPermission)){
			return $this->msg("target-immune", ["{target}" => $target->getName()]);
		}

		$this->targetName = $ltarget;
		$this->initiatorName = strtolower($initiator->getName());
		$this->votes = [$lname => true]; // Initiator auto-votes yes
		$this->cooldowns[$lname] = time();

		// Broadcast vote start
		$needed = $this->getNeededVotes();
		$broadcast = $this->msg("started", [
			"{target}" => $target->getDisplayName(),
			"{initiator}" => $initiator->getDisplayName(),
			"{duration}" => (string) $this->voteDuration,
			"{needed}" => (string) $needed,
		]);
		$this->plugin->getServer()->broadcastMessage(TextFormat::colorize($broadcast));

		// Schedule vote end
		$this->timerHandler = $this->plugin->getScheduler()->scheduleDelayedTask(
			new ClosureTask(function() : void{
				$this->endVote();
			}),
			$this->voteDuration * 20
		);

		return null;
	}

	public function castVote(Player $player, bool $yes) : ?string{
		if(!$this->isVoteActive()){
			return $this->msg("no-active-vote");
		}

		$lname = strtolower($player->getName());

		if($lname === $this->targetName){
			return $this->msg("cannot-vote-self");
		}

		if(isset($this->votes[$lname])){
			return $this->msg("already-voted");
		}

		$this->votes[$lname] = $yes;

		$word = $yes ? $this->msg("vote-yes-word", "yes") : $this->msg("vote-no-word", "no");
		return $this->msg("vote-registered", ["{vote}" => $word]);
	}

	public function cancelVote() : void{
		if($this->timerHandler !== null){
			$this->timerHandler->cancel();
			$this->timerHandler = null;
		}
		$this->targetName = null;
		$this->votes = [];
	}

	public function shutdown() : void{
		$this->cancelVote();
	}

	public function clearPlayer(string $playerName) : void{
		// If target disconnects, cancel vote
		if(strtolower($playerName) === $this->targetName){
			$broadcast = $this->msg("target-disconnected", ["{target}" => $playerName]);
			$this->plugin->getServer()->broadcastMessage(TextFormat::colorize($broadcast));
			$this->cancelVote();
		}
	}

	private function endVote() : void{
		if(!$this->isVoteActive()) return;

		$yes = 0;
		$no = 0;
		foreach($this->votes as $vote){
			if($vote) $yes++; else $no++;
		}

		$total = $yes + $no;
		$needed = $this->getNeededVotes();

		if($yes >= $needed){
			// Vote passed — mute the player
			$this->plugin->getServer()->broadcastMessage(TextFormat::colorize(
				$this->msg("passed", [
					"{target}" => $this->targetName,
					"{yes}" => (string) $yes,
					"{no}" => (string) $no,
					"{duration}" => $this->formatDuration($this->muteDuration),
				])
			));

			/** @var MuteManager $muteMgr */
			$muteMgr = $this->plugin->getMuteManager();
			$muteMgr->mute($this->targetName, $this->muteDuration);
		}else{
			// Vote failed
			$this->plugin->getServer()->broadcastMessage(TextFormat::colorize(
				$this->msg("failed", [
					"{target}" => $this->targetName,
					"{yes}" => (string) $yes,
					"{no}" => (string) $no,
					"{needed}" => (string) $needed,
				])
			));
		}

		$this->targetName = null;
		$this->votes = [];
		$this->timerHandler = null;
	}

	private function getNeededVotes() : int{
		$online = count($this->plugin->getServer()->getOnlinePlayers());
		return (int) ceil($online * $this->yesPercentage);
	}

	private function msg(string $key, array $replacements = []) : string{
		$msg = $this->messages->getNested("vote-mute." . $key, "");
		foreach($replacements as $search => $replace){
			$msg = str_replace($search, $replace, $msg);
		}
		return TextFormat::colorize($msg);
	}

	private function formatDuration(int $seconds) : string{
		if($seconds >= 3600) return intdiv($seconds, 3600) . "h";
		if($seconds >= 60) return intdiv($seconds, 60) . "m";
		return $seconds . "s";
	}
}
