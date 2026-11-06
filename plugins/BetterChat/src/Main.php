<?php

declare(strict_types=1);

namespace BetterChat;

use pocketmine\command\Command;
use pocketmine\command\CommandSender;
use pocketmine\plugin\PluginBase;
use pocketmine\utils\Config;
use BetterChat\command\BetterChatCommand;
use BetterChat\command\GroupCommand;
use BetterChat\listener\ChatListener;
use BetterChat\manager\BroadcastManager;
use BetterChat\manager\FilterManager;
use BetterChat\manager\GroupManager;
use BetterChat\manager\MinigameManager;
use BetterChat\manager\MuteManager;
use BetterChat\manager\SpamManager;
use BetterChat\manager\VoteMuteManager;

class Main extends PluginBase{

	private const CONFIG_VERSION = 2;

	private SpamManager $spamManager;
	private FilterManager $filterManager;
	private MuteManager $muteManager;
	private MinigameManager $minigameManager;
	private GroupManager $groupManager;
	private BroadcastManager $broadcastManager;
	private VoteMuteManager $voteMuteManager;
	private Config $messages;

	public function onEnable() : void{
		$this->saveDefaultConfig();
		$this->checkConfigVersion();
		$this->loadMessages();
		$this->initManagers();

		if($this->getConfig()->getNested("ranksystem.enabled", true) && !$this->isRankSystemLoaded()){
			$this->getLogger()->warning($this->messages->get("console-no-ranksystem", "RankSystem integration is enabled but the plugin is not installed. Skipping..."));
		}

		$this->getServer()->getPluginManager()->registerEvents(
			new ChatListener($this),
			$this
		);

		$this->getLogger()->info($this->messages->get("console-enabled", "BetterChat enabled!"));
	}

	public function onDisable() : void{
		$this->minigameManager->shutdown();
		$this->broadcastManager->shutdown();
		$this->voteMuteManager->shutdown();
		$this->groupManager->save();
		if($this->getConfig()->getNested("mute.persist", true)){
			$this->muteManager->save();
		}
	}

	private function initManagers() : void{
		$this->spamManager = new SpamManager($this->getConfig(), $this->messages);
		$this->filterManager = new FilterManager($this->getConfig());
		$this->muteManager = new MuteManager($this, $this->getConfig());
		$this->minigameManager = new MinigameManager($this, $this->getConfig(), $this->messages);
		$this->groupManager = new GroupManager($this, $this->getConfig(), $this->messages);
		$this->broadcastManager = new BroadcastManager($this, $this->getConfig());
		$this->voteMuteManager = new VoteMuteManager($this, $this->getConfig(), $this->messages);
	}

	public function isRankSystemLoaded() : bool{
		return $this->getServer()->getPluginManager()->getPlugin("RankSystem") !== null;
	}

	public function reload() : void{
		$this->minigameManager->shutdown();
		$this->broadcastManager->shutdown();
		$this->voteMuteManager->shutdown();
		$this->groupManager->save();
		$this->reloadConfig();
		$this->loadMessages();
		$this->initManagers();
	}

	public function getSpamManager() : SpamManager{
		return $this->spamManager;
	}

	public function getFilterManager() : FilterManager{
		return $this->filterManager;
	}

	public function getMuteManager() : MuteManager{
		return $this->muteManager;
	}

	public function getMinigameManager() : MinigameManager{
		return $this->minigameManager;
	}

	public function getGroupManager() : GroupManager{
		return $this->groupManager;
	}

	public function getBroadcastManager() : BroadcastManager{
		return $this->broadcastManager;
	}

	public function getVoteMuteManager() : VoteMuteManager{
		return $this->voteMuteManager;
	}

	public function getMessages() : Config{
		return $this->messages;
	}

	public function msg(string $key, mixed $default = "") : string{
		return (string) $this->messages->getNested($key, $default);
	}

	private function checkConfigVersion() : void{
		$userVersion = $this->getConfig()->get("config-version", 0);
		if($userVersion >= self::CONFIG_VERSION){
			return;
		}
		$this->getLogger()->notice("Config is outdated (v{$userVersion} -> v" . self::CONFIG_VERSION . "). Updating...");

		// Load the default config from resources
		$defaultPath = $this->getDataFolder() . "config.yml";
		$resourcePath = $this->getFile() . "resources/config.yml";
		$defaultConfig = new Config($resourcePath, Config::YAML);
		$defaultData = $defaultConfig->getAll();

		// Merge: user values preserved, new keys added
		$merged = $this->mergeConfig($this->getConfig()->getAll(), $defaultData);
		$merged["config-version"] = self::CONFIG_VERSION;

		// Back up old config, save merged
		rename($defaultPath, $defaultPath . ".old");
		$newConfig = new Config($defaultPath, Config::YAML);
		$newConfig->setAll($merged);
		$newConfig->save();

		// Reload config in memory
		$this->reloadConfig();
		$this->getLogger()->notice("Config updated! Old config saved as config.yml.old");
	}

	private function mergeConfig(array $user, array $default) : array{
		foreach($default as $key => $value){
			if(!array_key_exists($key, $user)){
				// New key from default - add it
				$user[$key] = $value;
			}elseif(is_array($value) && is_array($user[$key])){
				// Both are arrays - recurse to merge nested keys
				$user[$key] = $this->mergeConfig($user[$key], $value);
			}
			// If key exists in user config, keep user's value
		}
		return $user;
	}

	private function loadMessages() : void{
		$lang = $this->getConfig()->get("language", "en");
		$langFile = $this->getDataFolder() . "lang" . DIRECTORY_SEPARATOR . $lang . ".yml";
		if(!file_exists($langFile)){
			$this->saveResource("lang" . DIRECTORY_SEPARATOR . $lang . ".yml");
		}
		if(!file_exists($langFile)){
			// Fallback to English if requested language doesn't exist
			$langFile = $this->getDataFolder() . "lang" . DIRECTORY_SEPARATOR . "en.yml";
			if(!file_exists($langFile)){
				$this->saveResource("lang" . DIRECTORY_SEPARATOR . "en.yml");
			}
		}
		$this->messages = new Config($langFile, Config::YAML);
	}

	public function onCommand(CommandSender $sender, Command $command, string $label, array $args) : bool{
		if($command->getName() === "bchat"){
			return (new BetterChatCommand($this))->execute($sender, $args);
		}
		if($command->getName() === "group"){
			return (new GroupCommand($this))->execute($sender, $args);
		}
		if($command->getName() === "votemute"){
			return $this->handleVoteMute($sender, $args);
		}
		return false;
	}

	private function handleVoteMute(CommandSender $sender, array $args) : bool{
		if(!($sender instanceof \pocketmine\player\Player)){
			$sender->sendMessage(\pocketmine\utils\TextFormat::colorize("&cThis command can only be used in-game."));
			return true;
		}
		if(!$sender->hasPermission("betterchat.votemute")){
			$sender->sendMessage(\pocketmine\utils\TextFormat::colorize($this->msg("no-permission")));
			return true;
		}

		$voteMgr = $this->voteMuteManager;

		if(empty($args)){
			$sender->sendMessage(\pocketmine\utils\TextFormat::colorize($this->msg("vote-mute.usage")));
			return true;
		}

		$first = strtolower($args[0]);

		// Voting yes/no on active vote
		if($first === "yes" || $first === "no"){
			$error = $voteMgr->castVote($sender, $first === "yes");
			if($error !== null){
				$sender->sendMessage(\pocketmine\utils\TextFormat::colorize($error));
			}
			return true;
		}

		// Start a new vote
		$error = $voteMgr->startVote($sender, $args[0]);
		if($error !== null){
			$sender->sendMessage(\pocketmine\utils\TextFormat::colorize($error));
		}
		return true;
	}
}
