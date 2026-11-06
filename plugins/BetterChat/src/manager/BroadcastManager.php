<?php

declare(strict_types=1);

namespace BetterChat\manager;

use pocketmine\plugin\PluginBase;
use pocketmine\scheduler\ClosureTask;
use pocketmine\scheduler\TaskHandler;
use pocketmine\utils\Config;
use pocketmine\utils\TextFormat;

class BroadcastManager{

	private bool $enabled;
	private int $interval;
	private string $prefix;
	/** @var array<string, string[]> key => lines[] */
	private array $entries = [];
	/** @var string[] ordered list of entry keys */
	private array $entryKeys = [];
	private int $currentIndex = 0;
	private ?TaskHandler $taskHandler = null;

	public function __construct(
		PluginBase $plugin,
		Config $config
	){
		$p = "broadcast.";
		$this->enabled = $config->getNested($p . "enabled", true);
		$this->interval = $config->getNested($p . "interval", 120);
		$this->prefix = $config->getNested($p . "prefix", "&8[&eBroadcast&8] &r");

		$dataFile = $plugin->getDataFolder() . "broadcasts.yml";
		if(!file_exists($dataFile)){
			$plugin->saveResource("broadcasts.yml");
		}
		$data = new Config($dataFile, Config::YAML);
		foreach($data->getAll() as $key => $lines){
			if(is_array($lines) && !empty($lines)){
				$this->entries[$key] = $lines;
				$this->entryKeys[] = $key;
			}
		}

		if($this->enabled && !empty($this->entryKeys) && $this->interval > 0){
			$this->schedule($plugin);
		}
	}

	public function isEnabled() : bool{
		return $this->enabled;
	}

	public function shutdown() : void{
		if($this->taskHandler !== null){
			$this->taskHandler->cancel();
			$this->taskHandler = null;
		}
	}

	private function schedule(PluginBase $plugin) : void{
		$this->taskHandler = $plugin->getScheduler()->scheduleRepeatingTask(
			new ClosureTask(function() use($plugin) : void{
				$this->sendNext($plugin);
			}),
			$this->interval * 20
		);
	}

	private function sendNext(PluginBase $plugin) : void{
		if(empty($this->entryKeys)) return;

		$key = $this->entryKeys[$this->currentIndex];
		$lines = $this->entries[$key];

		foreach($lines as $line){
			$plugin->getServer()->broadcastMessage(TextFormat::colorize($this->prefix . $line));
		}

		$this->currentIndex++;
		if($this->currentIndex >= count($this->entryKeys)){
			$this->currentIndex = 0;
		}
	}
}
