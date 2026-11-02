<?php
declare(strict_types=1);

namespace pocketmine\example;

use pocketmine\plugin\PluginBase;
use pocketmine\event\Listener;
use pocketmine\event\player\PlayerJoinEvent;

class Main extends PluginBase implements Listener {
    public function onEnable(): void {
        $this->getLogger()->info("ExamplePlugin has been successfully enabled via Rust Plugin Bridge!");
    }

    public function onPlayerJoin(PlayerJoinEvent $event): void {
        $player = $event->getPlayer();
        $this->getLogger()->info("Player joined: " . $player->getName());
        $player->sendMessage("Welcome to PocketMine-Rust Server!");
    }
}
