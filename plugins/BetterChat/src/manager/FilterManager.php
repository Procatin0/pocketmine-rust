<?php

declare(strict_types=1);

namespace BetterChat\manager;

use pocketmine\utils\Config;

class FilterManager{

	private bool $enabled;
	private string $bypassPermission;
	/** @var string[] */
	private array $blockedWords;
	private string $replacement;

	public function __construct(Config $config){
		$this->enabled = $config->getNested("word-filter.enabled", true);
		$this->bypassPermission = $config->getNested("word-filter.bypass-permission", "betterchat.bypass.filter");
		$this->blockedWords = $config->getNested("word-filter.blocked-words", []);
		$this->replacement = $config->getNested("word-filter.replacement", "***");
	}

	public function isEnabled() : bool{
		return $this->enabled;
	}

	public function getBypassPermission() : string{
		return $this->bypassPermission;
	}

	public function filter(string $message) : string{
		foreach($this->blockedWords as $word){
			$word = (string) $word;
			if($word === "") continue;
			$pattern = '/' . preg_quote($word, '/') . '/i';
			$message = preg_replace($pattern, $this->replacement, $message);
		}
		return $message;
	}
}
