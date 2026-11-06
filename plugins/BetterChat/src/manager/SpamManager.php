<?php

declare(strict_types=1);

namespace BetterChat\manager;

use pocketmine\player\Player;
use pocketmine\utils\Config;

class SpamManager{

	/** @var array<string, float> player name => last message timestamp */
	private array $lastMessageTime = [];
	/** @var array<string, string> player name => last message content */
	private array $lastMessage = [];
	/** @var array<string, array<string, float>> player name => [command => last use timestamp] */
	private array $lastCommandTime = [];

	private bool $enabled;
	private string $bypassPermission;
	private int $cooldown;
	private bool $blockDuplicates;
	private int $maxLength;
	private int $capsPercentage;
	private int $capsMinLength;
	private string $cooldownMessage;
	private string $duplicateMessage;
	private string $tooLongMessage;
	private string $tooMuchCapsMessage;

	/** @var array<string, int> command name => cooldown seconds */
	private array $commandCooldowns = [];
	private bool $commandCooldownsEnabled;
	private string $commandBypassPermission;
	private string $commandCooldownMessage;

	public function __construct(Config $config, Config $messages){
		$s = "anti-spam.";
		$this->enabled = $config->getNested($s . "enabled", true);
		$this->bypassPermission = $config->getNested($s . "bypass-permission", "betterchat.bypass.spam");
		$this->cooldown = $config->getNested($s . "cooldown", 3);
		$this->blockDuplicates = $config->getNested($s . "block-duplicates", true);
		$this->maxLength = $config->getNested($s . "max-length", 100);
		$this->capsPercentage = $config->getNested($s . "caps-percentage", 70);
		$this->capsMinLength = $config->getNested($s . "caps-min-length", 4);
		$this->cooldownMessage = $messages->getNested("spam.cooldown", "&cSlow down! Wait {time}s.");
		$this->duplicateMessage = $messages->getNested("spam.duplicate", "&cDuplicate message.");
		$this->tooLongMessage = $messages->getNested("spam.too-long", "&cMessage too long! Max {max} chars.");
		$this->tooMuchCapsMessage = $messages->getNested("spam.too-much-caps", "&cToo many caps!");

		// Command cooldowns
		$cc = "command-cooldowns.";
		$this->commandCooldownsEnabled = $config->getNested($cc . "enabled", true);
		$this->commandBypassPermission = $config->getNested($cc . "bypass-permission", "betterchat.bypass.spam");
		$this->commandCooldownMessage = $messages->getNested("spam.command-cooldown", "&cWait {time}s before using this command again.");
		$cmds = $config->getNested($cc . "commands", []);
		foreach($cmds as $entry){
			if(isset($entry["command"]) && isset($entry["cooldown"])){
				$this->commandCooldowns[strtolower($entry["command"])] = (int) $entry["cooldown"];
			}
		}
	}

	public function isEnabled() : bool{
		return $this->enabled;
	}

	public function getBypassPermission() : string{
		return $this->bypassPermission;
	}

	/**
	 * Check if a message passes spam filters.
	 * @return string|null Error message if blocked, null if allowed.
	 */
	public function check(Player $player, string $message) : ?string{
		$name = strtolower($player->getName());
		$now = microtime(true);

		// Cooldown check
		if($this->cooldown > 0 && isset($this->lastMessageTime[$name])){
			$elapsed = $now - $this->lastMessageTime[$name];
			if($elapsed < $this->cooldown){
				$remaining = (string) ceil($this->cooldown - $elapsed);
				return str_replace("{time}", $remaining, $this->cooldownMessage);
			}
		}

		// Length check
		if($this->maxLength > 0 && strlen($message) > $this->maxLength){
			return str_replace("{max}", (string) $this->maxLength, $this->tooLongMessage);
		}

		// Duplicate check
		if($this->blockDuplicates && isset($this->lastMessage[$name])){
			if(strtolower($message) === strtolower($this->lastMessage[$name])){
				return $this->duplicateMessage;
			}
		}

		// Caps check
		if($this->capsPercentage > 0 && strlen($message) >= $this->capsMinLength){
			$upper = 0;
			$total = 0;
			for($i = 0, $len = strlen($message); $i < $len; $i++){
				$ch = $message[$i];
				if(ctype_alpha($ch)){
					$total++;
					if(ctype_upper($ch)){
						$upper++;
					}
				}
			}
			if($total > 0 && ($upper / $total * 100) >= $this->capsPercentage){
				return $this->tooMuchCapsMessage;
			}
		}

		// Update tracking only after all checks pass
		$this->lastMessageTime[$name] = $now;
		$this->lastMessage[$name] = $message;

		return null;
	}

	/**
	 * Check command cooldown.
	 * @return string|null Error message if blocked, null if allowed.
	 */
	public function checkCommandCooldown(Player $player, string $commandName) : ?string{
		if(!$this->commandCooldownsEnabled) return null;
		if(empty($this->commandCooldowns)) return null;

		$cmd = strtolower($commandName);
		if(!isset($this->commandCooldowns[$cmd])) return null;

		$name = strtolower($player->getName());
		$now = microtime(true);
		$cd = $this->commandCooldowns[$cmd];

		if(isset($this->lastCommandTime[$name][$cmd])){
			$elapsed = $now - $this->lastCommandTime[$name][$cmd];
			if($elapsed < $cd){
				$remaining = (string) ceil($cd - $elapsed);
				return str_replace("{time}", $remaining, $this->commandCooldownMessage);
			}
		}

		// Update tracking
		if(!isset($this->lastCommandTime[$name])){
			$this->lastCommandTime[$name] = [];
		}
		$this->lastCommandTime[$name][$cmd] = $now;

		return null;
	}

	public function getCommandBypassPermission() : string{
		return $this->commandBypassPermission;
	}

	public function isCommandCooldownsEnabled() : bool{
		return $this->commandCooldownsEnabled;
	}

	/** Call when a player quits to free memory */
	public function clearPlayer(string $playerName) : void{
		$name = strtolower($playerName);
		unset($this->lastMessageTime[$name], $this->lastMessage[$name], $this->lastCommandTime[$name]);
	}
}
