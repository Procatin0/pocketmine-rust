<?php

declare(strict_types=1);

namespace BetterChat;

use pocketmine\player\chat\ChatFormatter as ChatFormatterInterface;

class ChatFormatter implements ChatFormatterInterface{

	/**
	 * @param string               $format       The format string with placeholders
	 * @param array<string, string> $placeholders Key-value map of placeholder replacements
	 */
	public function __construct(
		private string $format,
		private array $placeholders
	){}

	public function format(string $username, string $message) : string{
		$replacements = array_merge($this->placeholders, [
			"player" => $username,
			"message" => $message,
		]);
		$result = $this->format;
		foreach($replacements as $key => $value){
			$result = str_replace("{" . $key . "}", $value, $result);
		}
		return $result;
	}
}
