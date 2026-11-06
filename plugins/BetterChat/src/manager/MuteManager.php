<?php

declare(strict_types=1);

namespace BetterChat\manager;

use pocketmine\player\Player;
use pocketmine\plugin\PluginBase;
use pocketmine\utils\Config;

class MuteManager{

	/** @var array<string, int> lowercase player name => expiry timestamp (0 = permanent) */
	private array $mutedPlayers = [];
	/** @var array<string, string> lowercase player name => custom prefix */
	private array $customPrefixes = [];
	/** @var array<string, string> lowercase player name => custom suffix */
	private array $customSuffixes = [];

	private bool $persist;

	public function __construct(
		private PluginBase $plugin,
		Config $config
	){
		$this->persist = $config->getNested("mute.persist", true);
		$this->load();
	}

	public function isMuted(Player $player) : bool{
		return $this->checkMuted(strtolower($player->getName()));
	}

	public function isPlayerMuted(string $playerName) : bool{
		return $this->checkMuted(strtolower($playerName));
	}

	/**
	 * @param int $duration Seconds (0 = permanent)
	 */
	public function mute(string $playerName, int $duration = 0) : void{
		$expiry = $duration > 0 ? time() + $duration : 0;
		$this->mutedPlayers[strtolower($playerName)] = $expiry;
		if($this->persist) $this->save();
	}

	/**
	 * Get remaining mute time in seconds (0 = permanent, -1 = not muted).
	 */
	public function getRemainingTime(string $playerName) : int{
		$key = strtolower($playerName);
		if(!isset($this->mutedPlayers[$key])) return -1;
		$expiry = $this->mutedPlayers[$key];
		if($expiry === 0) return 0;
		$remaining = $expiry - time();
		if($remaining <= 0){
			unset($this->mutedPlayers[$key]);
			if($this->persist) $this->save();
			return -1;
		}
		return $remaining;
	}

	public function unmute(string $playerName) : void{
		unset($this->mutedPlayers[strtolower($playerName)]);
		if($this->persist) $this->save();
	}

	public function getMutedPlayers() : array{
		return array_keys($this->mutedPlayers);
	}

	public function setCustomPrefix(string $playerName, string $prefix) : void{
		$this->customPrefixes[strtolower($playerName)] = $prefix;
		if($this->persist) $this->save();
	}

	public function getCustomPrefix(Player $player) : ?string{
		return $this->customPrefixes[strtolower($player->getName())] ?? null;
	}

	public function removeCustomPrefix(string $playerName) : void{
		unset($this->customPrefixes[strtolower($playerName)]);
		if($this->persist) $this->save();
	}

	public function setCustomSuffix(string $playerName, string $suffix) : void{
		$this->customSuffixes[strtolower($playerName)] = $suffix;
		if($this->persist) $this->save();
	}

	public function getCustomSuffix(Player $player) : ?string{
		return $this->customSuffixes[strtolower($player->getName())] ?? null;
	}

	public function removeCustomSuffix(string $playerName) : void{
		unset($this->customSuffixes[strtolower($playerName)]);
		if($this->persist) $this->save();
	}

	public function save() : void{
		$dataFile = $this->plugin->getDataFolder() . "data.yml";
		$config = new Config($dataFile, Config::YAML);
		$config->set("muted", $this->mutedPlayers);
		$config->set("prefixes", $this->customPrefixes);
		$config->set("suffixes", $this->customSuffixes);
		$config->save();
	}

	private function load() : void{
		$dataFile = $this->plugin->getDataFolder() . "data.yml";
		if(!file_exists($dataFile)) return;
		$config = new Config($dataFile, Config::YAML);
		$this->mutedPlayers = $config->get("muted", []);
		$this->customPrefixes = $config->get("prefixes", []);
		$this->customSuffixes = $config->get("suffixes", []);
	}

	private function checkMuted(string $key) : bool{
		if(!isset($this->mutedPlayers[$key])) return false;
		$expiry = $this->mutedPlayers[$key];
		if($expiry === 0) return true;
		if(time() >= $expiry){
			unset($this->mutedPlayers[$key]);
			if($this->persist) $this->save();
			return false;
		}
		return true;
	}
}
