<?php

declare(strict_types=1);

namespace economylite;

use economylite\command\EcoCommand;
use economylite\command\MoneyCommand;
use economylite\command\PayCommand;
use pocketmine\command\CommandExecutor;
use pocketmine\event\Listener;
use pocketmine\event\player\PlayerJoinEvent;
use pocketmine\plugin\PluginBase;
use pocketmine\scheduler\ClosureTask;

/**
 * Economía mínima con persistencia JSON.
 *
 * Demuestra Config, getDataFolder(), Scheduler, permisos,
 * getPlayerExact(), getOnlinePlayers() y mensajes a jugadores.
 */
final class Main extends PluginBase implements Listener{

	private const AUTOSAVE_TICKS = 1200;

	/** @var array<string, float> clave en minúsculas => saldo */
	private array $balances = [];

	public function onEnable() : void{
		$this->saveDefaultConfig();
		$this->getServer()->getPluginManager()->registerEvents($this, $this);
		$this->loadBalances();

		// Crea cuentas para jugadores que ya estuvieran conectados (recarga, reload, etc.).
		foreach($this->getServer()->getOnlinePlayers() as $player){
			$this->getOrCreateBalance($player->getName());
		}

		$this->registerCommands();

		$this->getScheduler()->scheduleRepeatingTask(new ClosureTask(function() : void{
			$this->saveBalances();
		}), self::AUTOSAVE_TICKS);

		$this->getLogger()->info(
			"EconomyLite habilitado. " . count($this->balances) . " cuenta(s), saldo inicial " .
			$this->formatMoney($this->getStartingMoney()) . "."
		);
	}

	public function onDisable() : void{
		$this->saveBalances();
		$this->getLogger()->info("EconomyLite deshabilitado; saldos guardados.");
	}

	public function onJoin(PlayerJoinEvent $event) : void{
		$player = $event->getPlayer();
		$balance = $this->getOrCreateBalance($player->getName());
		$this->getLogger()->info("EconomyLite: cuenta de " . $player->getName() . " lista (" . $this->formatMoney($balance) . ").");
	}

	private function registerCommands() : void{
		/** @var array<string, CommandExecutor> $executors */
		$executors = [
			"money" => new MoneyCommand($this),
			"pay" => new PayCommand($this),
			"eco" => new EcoCommand($this)
		];

		foreach($executors as $name => $executor){
			$command = $this->getCommand($name);
			if($command === null){
				$this->getLogger()->warning("No se encontró el comando /{$name} en plugin.yml");
				continue;
			}

			$command->setExecutor($executor);
		}
	}

	private function getBalancesPath() : string{
		return rtrim($this->getDataFolder(), "/\\") . "/balances.json";
	}

	private function loadBalances() : void{
		$path = $this->getBalancesPath();
		if(!is_file($path)){
			return;
		}

		$content = file_get_contents($path);
		if($content === false){
			$this->getLogger()->warning("No se pudo leer " . $path);
			return;
		}

		$decoded = json_decode($content, true);
		if(!is_array($decoded)){
			$this->getLogger()->warning("balances.json no contiene un objeto JSON válido; se ignorará.");
			return;
		}

		$loaded = 0;
		foreach($decoded as $name => $amount){
			if(!is_string($name) || (!is_int($amount) && !is_float($amount) && !(is_string($amount) && is_numeric($amount)))){
				continue;
			}

			$key = $this->normalizeName($name);
			if($key === ""){
				continue;
			}

			$this->balances[$key] = max(0.0, round((float) $amount, 2));
			$loaded++;
		}

		$this->getLogger()->info("EconomyLite: cargadas " . $loaded . " cuenta(s) desde " . $path);
	}

	public function saveBalances() : void{
		$path = $this->getBalancesPath();
		$json = json_encode($this->balances, JSON_PRETTY_PRINT | JSON_UNESCAPED_UNICODE | JSON_UNESCAPED_SLASHES);
		if($json === false){
			$this->getLogger()->error("No se pudieron serializar los saldos a JSON.");
			return;
		}

		if(file_put_contents($path, $json . PHP_EOL) === false){
			$this->getLogger()->error("No se pudo escribir " . $path);
		}
	}

	private function normalizeName(string $name) : string{
		return strtolower(trim($name));
	}

	public function getStartingMoney() : float{
		return max(0.0, round((float) $this->getConfig()->get("starting-money", 100), 2));
	}

	public function getCurrencySymbol() : string{
		$symbol = $this->getConfig()->get("currency-symbol", "$");
		return is_string($symbol) && $symbol !== "" ? $symbol : "$";
	}

	public function formatMoney(float $amount) : string{
		return $this->getCurrencySymbol() . number_format($amount, 2);
	}

	public function getBalance(string $name) : ?float{
		$key = $this->normalizeName($name);
		return $this->balances[$key] ?? null;
	}

	public function getOrCreateBalance(string $name) : float{
		$key = $this->normalizeName($name);
		if(!array_key_exists($key, $this->balances)){
			$this->balances[$key] = $this->getStartingMoney();
		}

		return $this->balances[$key];
	}

	public function findAccountName(string $name) : ?string{
		$key = $this->normalizeName($name);
		return array_key_exists($key, $this->balances) ? $name : null;
	}

	public function setBalance(string $name, float $amount) : float{
		$key = $this->normalizeName($name);
		$this->balances[$key] = max(0.0, round($amount, 2));
		$this->saveBalances();

		return $this->balances[$key];
	}

	public function addBalance(string $name, float $amount) : float{
		return $this->setBalance($name, $this->getOrCreateBalance($name) + $amount);
	}

	public function subtractBalance(string $name, float $amount) : ?float{
		$current = $this->getBalance($name);
		if($current === null || $current < $amount){
			return null;
		}

		return $this->setBalance($name, $current - $amount);
	}

	public function transfer(string $from, string $to, float $amount) : bool{
		$fromKey = $this->normalizeName($from);
		$toKey = $this->normalizeName($to);
		if($fromKey === "" || $toKey === "" || $fromKey === $toKey){
			return false;
		}

		$fromBalance = $this->getOrCreateBalance($from);
		$toBalance = $this->getOrCreateBalance($to);
		if($fromBalance < $amount){
			return false;
		}

		$this->balances[$fromKey] = round($fromBalance - $amount, 2);
		$this->balances[$toKey] = round($toBalance + $amount, 2);
		$this->saveBalances();

		return true;
	}

	/**
	 * @return string[]
	 */
	public function getOnlinePlayerNames() : array{
		$names = [];
		foreach($this->getServer()->getOnlinePlayers() as $player){
			$names[] = $player->getName();
		}

		sort($names);
		return $names;
	}
}
