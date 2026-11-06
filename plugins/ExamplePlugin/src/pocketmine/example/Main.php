<?php

declare(strict_types=1);

namespace pocketmine\example;

use pocketmine\command\Command;
use pocketmine\command\CommandSender;
use pocketmine\event\Listener;
use pocketmine\event\player\PlayerJoinEvent;
use pocketmine\player\Player;
use pocketmine\plugin\PluginBase;
use pocketmine\utils\TextFormat;

/**
 * Plugin mínimo de referencia para el puente PHP de PocketMine-Rust.
 *
 * Demuestra:
 * - ciclo de vida de PluginBase;
 * - registro explícito de listeners;
 * - lectura de configuración con saveDefaultConfig()/getConfig();
 * - ejecución de un comando declarado en plugin.yml.
 */
final class Main extends PluginBase implements Listener{

	public function onEnable() : void{
		$this->saveDefaultConfig();
		$this->getServer()->getPluginManager()->registerEvents($this, $this);

		$command = $this->getCommand("hello");
		if($command !== null){
			$command->setExecutor($this);
		}else{
			$this->getLogger()->warning("No se encontró el comando /hello en plugin.yml");
		}

		$this->getLogger()->info("ExamplePlugin habilitado. Datos en: " . $this->getDataFolder());
		$this->getLogger()->info("Mensaje de bienvenida: " . (string) $this->getConfig()->get("welcome-message", "Welcome to PocketMine-Rust Server!"));
	}

	public function onDisable() : void{
		$this->getLogger()->info("ExamplePlugin deshabilitado.");
	}

	public function onPlayerJoin(PlayerJoinEvent $event) : void{
		$player = $event->getPlayer();
		$welcomeMessage = (string) $this->getConfig()->get("welcome-message", "Welcome to PocketMine-Rust Server!");

		$this->getLogger()->info("Jugador conectado: " . $player->getName());
		$player->sendMessage(TextFormat::GREEN . $welcomeMessage);
	}

	public function onCommand(CommandSender $sender, Command $command, string $label, array $args) : bool{
		if($command->getName() !== "hello"){
			return false;
		}

		if(!$sender->hasPermission("exampleplugin.command.hello")){
			$sender->sendMessage(TextFormat::RED . "No tienes permiso para usar /hello.");
			return true;
		}

		$name = $sender instanceof Player ? $sender->getName() : "Console";
		$message = "Hello, " . $name . "!";
		$sender->sendMessage(TextFormat::AQUA . $message);

		if((bool) $this->getConfig()->get("broadcast-hello", false)){
			foreach($this->getServer()->getOnlinePlayers() as $onlinePlayer){
				if($onlinePlayer->getName() !== $name){
					$onlinePlayer->sendMessage(TextFormat::AQUA . $message);
				}
			}
			$this->getLogger()->info("Hello difundido por " . $name);
		}

		return true;
	}
}
