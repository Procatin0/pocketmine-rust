<?php

declare(strict_types=1);

namespace chatguard;

use pocketmine\event\Listener;
use pocketmine\event\player\PlayerChatEvent;
use pocketmine\plugin\PluginBase;
use pocketmine\scheduler\ClosureTask;

/**
 * Filtro de chat con lista de palabras configurable.
 *
 * Demuestra la mutación de un evento cancelable con prioridad HIGH,
 * el registro de un comando propio mediante CommandMap y una tarea
 * repetitiva del scheduler.
 */
final class Main extends PluginBase implements Listener{

	private bool $filterEnabled = true;

	/** @var string[] */
	private array $blockedWords = [];

	public function onEnable() : void{
		$this->saveDefaultConfig();
		$this->blockedWords = $this->readBlockedWords();

		$this->getServer()->getPluginManager()->registerEvents($this, $this);
		$this->getServer()->getCommandMap()->register("chatguard", new ChatGuardCommand($this));

		$this->getScheduler()->scheduleRepeatingTask(new ClosureTask(function() : void{
			$this->getLogger()->info("ChatGuard heartbeat");
		}), 200);

		$this->getLogger()->info("ChatGuard habilitado. Filtro " . ($this->filterEnabled ? "activo" : "inactivo") . " con " . count($this->blockedWords) . " palabra(s).");
	}

	public function onDisable() : void{
		$this->getLogger()->info("ChatGuard deshabilitado.");
	}

	/**
	 * Reemplaza las palabras prohibidas antes de que el mensaje se difunda.
	 *
	 * @priority HIGH
	 * @ignoreCancelled true
	 */
	public function onChat(PlayerChatEvent $event) : void{
		if(!$this->filterEnabled){
			return;
		}

		$player = $event->getPlayer();
		if($player->hasPermission("chatguard.bypass")){
			return;
		}

		$original = $event->getMessage();
		$filtered = $original;

		foreach($this->blockedWords as $word){
			$pattern = '/\b' . preg_quote($word, '/') . '\b/iu';
			$filtered = preg_replace($pattern, "***", $filtered) ?? $filtered;
		}

		if($filtered === $original){
			return;
		}

		$event->setMessage($filtered);
		$this->getLogger()->info("ChatGuard filtró a " . $player->getName() . ": \"" . $original . "\" -> \"" . $filtered . "\"");
	}

	public function isFilterEnabled() : bool{
		return $this->filterEnabled;
	}

	public function setFilterEnabled(bool $enabled) : void{
		$this->filterEnabled = $enabled;
	}

	/**
	 * @return string[]
	 */
	public function getBlockedWords() : array{
		return $this->blockedWords;
	}

	/**
	 * @return string[]
	 */
	private function readBlockedWords() : array{
		$configured = $this->getConfig()->get("blocked-words", ["badword", "spam"]);
		if(!is_array($configured)){
			return ["badword", "spam"];
		}

		$words = [];
		foreach($configured as $word){
			$word = strtolower(trim((string) $word));
			if($word !== ""){
				$words[] = $word;
			}
		}

		return $words;
	}
}
