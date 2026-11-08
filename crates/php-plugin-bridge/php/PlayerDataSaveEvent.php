<?php

declare(strict_types=1);

namespace pocketmine\event\player;

use pocketmine\event\Cancellable;
use pocketmine\event\Event;
use pocketmine\event\EventData;
use pocketmine\player\Player;

if (!class_exists(PlayerDataSaveEvent::class)) {
    /**
     * Disparado antes de persistir un jugador (`Player::save()`).
     * Cancelarlo impide el guardado; el bridge Java lo dispara en leaveApi.
     */
    class PlayerDataSaveEvent extends Event implements Cancellable
    {
        protected Player $player;

        public function __construct(Player $player)
        {
            $this->player = $player;
        }

        public function getPlayer(): Player
        {
            return $this->player;
        }

        public static function fromArray(array $data): Event
        {
            $event = new static(EventData::player($data['player'] ?? null));
            $event->applyCancelled($data);
            return $event;
        }

        public function toArray(): array
        {
            return [
                'player' => EventData::playerToArray($this->player),
                'cancelled' => $this->isCancelled(),
            ];
        }
    }
}
