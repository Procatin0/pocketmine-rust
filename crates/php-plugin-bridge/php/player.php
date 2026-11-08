<?php

declare(strict_types=1);

namespace pocketmine\player {

    use pocketmine\command\CommandSender;
    use pocketmine\inventory\PlayerInventory;
    use pocketmine\math\Position;
    use pocketmine\math\Vector3;
    use pocketmine\permission\Permission;
    use pocketmine\permission\PermissionManager;
    use pocketmine\utils\TextFormat;
    use pocketmine\world\Location;
    use pocketmine\world\World;

    class Player implements CommandSender
    {
        public string $name;
        public string $displayName;
        public string $address;
        public int $port;
        public int|string $runtimeId;
        public int|string $uniqueId;
        public int $ping;
        public int $gamemode;
        public string $worldName;
        public float $x;
        public float $y;
        public float $z;
        public float $yaw;
        public float $pitch;
        public float $health;
        public float $maxHealth;
        public bool $flying;
        public float $xp;
        public int $xpLevel;

        private PlayerInventory $inventory;
        private PlayerInventory $enderInventory;
        private PlayerInventory $cursorInventory;
        private PlayerInventory $craftingGrid;
        private PlayerInventory $creativeInventory;
        private bool $online = true;
        private array $permissions = [];
        private ?bool $opOverride = null;

        /** Fase A1: `hiddenPlayers` (canSee). Clave: lowercase name. */
        private array $hiddenPlayers = [];
        /** Fase A1: locale PM (`en_US` por defecto). */
        private string $locale = 'en_US';
        /** Fase A1: flags de movimiento (toggle* locales). */
        private bool $sprinting = false;
        private bool $sneaking = false;
        private bool $sneakPressed = false;
        private bool $gliding = false;
        private bool $swimming = false;
        /** Fase A1: posición de la cama si duerme (`isSleeping`). */
        private mixed $sleeping = null;
        /** Fase A1: hambre/aire para `canEat`/`canBreathe` (simplificado). */
        private float $food = 20.0;
        private float $saturation = 20.0;
        private int $air = 300;

        /** `Player::getFirstPlayed()`/`getLastPlayed()` (microtiempo float). */
        private float $firstPlayed = 0.0;
        private float $lastPlayed = 0.0;
        /** ¿Existía `players/<name>.json` antes de esta sesión? */
        private ?bool $hadSavedData = null;
        private bool $playedLoaded = false;

        public function __construct(array|string $data = ['name' => 'Steve'])
        {
            if (is_string($data)) {
                $data = ['name' => $data];
            }

            $this->name = (string)($data['name'] ?? $data['username'] ?? 'Steve');
            $this->displayName = (string)($data['display_name'] ?? $data['displayName'] ?? $this->name);
            $this->address = (string)($data['address'] ?? $data['ip'] ?? '127.0.0.1');
            $this->port = (int)($data['port'] ?? 19132);
            $this->runtimeId = (int)($data['runtime_id'] ?? $data['runtimeId'] ?? 1);
            $uniqueId = $data['unique_id'] ?? $data['uniqueId'] ?? $data['xuid'] ?? $this->name;
            $this->uniqueId = is_numeric($uniqueId) ? (int)$uniqueId : (string)$uniqueId;
            $this->ping = (int)($data['ping'] ?? 0);
            $this->gamemode = (int)($data['gamemode'] ?? 0);
            $this->worldName = (string)($data['world'] ?? 'world');
            $this->x = (float)($data['x'] ?? 0.0);
            $this->y = (float)($data['y'] ?? 64.0);
            $this->z = (float)($data['z'] ?? 0.0);
            $this->yaw = (float)($data['yaw'] ?? 0.0);
            $this->pitch = (float)($data['pitch'] ?? 0.0);
            $this->health = (float)($data['health'] ?? 20.0);
            $this->maxHealth = (float)($data['max_health'] ?? 20.0);
            $this->flying = (bool)($data['flying'] ?? false);
            $this->xp = (float)($data['xp'] ?? 0.0);
            $this->xpLevel = (int)($data['xp_level'] ?? $data['xpLevel'] ?? 0);
            $this->online = (bool)($data['online'] ?? true);
            $this->locale = (string)($data['locale'] ?? 'en_US');
            $this->sprinting = (bool)($data['sprinting'] ?? false);
            $this->sneaking = (bool)($data['sneaking'] ?? false);
            $this->gliding = (bool)($data['gliding'] ?? false);
            $this->swimming = (bool)($data['swimming'] ?? false);
            $this->food = (float)($data['food'] ?? 20.0);
            $this->saturation = (float)($data['saturation'] ?? 20.0);
            $this->air = (int)($data['air'] ?? 300);
            $this->firstPlayed = (float)($data['first_played'] ?? $data['firstPlayed'] ?? 0.0);
            $this->lastPlayed = (float)($data['last_played'] ?? $data['lastPlayed'] ?? 0.0);
            $hasPlayedBefore = $data['has_played_before'] ?? $data['hasPlayedBefore'] ?? null;
            if (is_bool($hasPlayedBefore)) {
                $this->hadSavedData = $hasPlayedBefore;
                $this->playedLoaded = true;
            }
            if ($this->firstPlayed > 0.0 || $this->lastPlayed > 0.0) {
                $this->playedLoaded = true;
                if ($this->hadSavedData === null) {
                    $this->hadSavedData = $this->lastPlayed > $this->firstPlayed + 0.001;
                }
            }

            if (isset($data['permissions']) && is_array($data['permissions'])) {
                $this->permissions = $data['permissions'];
            }
            if (array_key_exists('is_op', $data) || array_key_exists('op', $data)) {
                $this->opOverride = (bool)($data['is_op'] ?? $data['op']);
            }
            if (isset($data['hidden_players']) && is_array($data['hidden_players'])) {
                foreach ($data['hidden_players'] as $name) {
                    $this->hiddenPlayers[strtolower((string)$name)] = true;
                }
            }

            $this->inventory = new PlayerInventory($this->name);
            // Fase A1: stub inventories (ver getters; sync TODO).
            $this->enderInventory = new PlayerInventory($this->name . '#ender');
            $this->cursorInventory = new PlayerInventory($this->name . '#cursor');
            $this->craftingGrid = new PlayerInventory($this->name . '#crafting');
            $this->creativeInventory = new PlayerInventory($this->name . '#creative');
        }

        public function getName(): string
        {
            return $this->name;
        }

        public function getLowerCaseName(): string
        {
            return strtolower($this->name);
        }

        public function getDisplayName(): string
        {
            return $this->displayName;
        }

        public function setDisplayName(string $name): void
        {
            $this->displayName = $name;
        }

        public function getRawDisplayName(): string
        {
            return TextFormat::clean($this->displayName);
        }

        public function getAddress(): string
        {
            return $this->address;
        }

        public function getPort(): int
        {
            return $this->port;
        }

        public function getPing(): int
        {
            return $this->ping;
        }

        public function getPingTicks(): int
        {
            return (int)round($this->ping / 50);
        }

        public function getGamemode(): int
        {
            return $this->gamemode;
        }

        public function setGamemode(int $gamemode): void
        {
            $this->gamemode = $gamemode;
            $this->addAction('set_gamemode', ['target_player' => $this->name, 'gamemode' => $gamemode]);
        }

        public function getX(): float
        {
            return $this->x;
        }

        public function getY(): float
        {
            return $this->y;
        }

        public function getZ(): float
        {
            return $this->z;
        }

        public function getYaw(): float
        {
            return $this->yaw;
        }

        public function getPitch(): float
        {
            return $this->pitch;
        }

        public function getPosition(): Position
        {
            return new Position($this->x, $this->y, $this->z, $this->worldName);
        }

        public function getLocation(): Location
        {
            return new Location($this->x, $this->y, $this->z, $this->yaw, $this->pitch, $this->worldName);
        }

        public function getWorld(): World
        {
            return \pocketmine\Server::getInstance()->getWorldManager()->getWorldByName($this->worldName);
        }

        public function getHealth(): float
        {
            return $this->health;
        }

        public function setHealth(float $health): void
        {
            $this->health = max(0.0, min($this->maxHealth, $health));
            // Fase A1: `PluginAction::SetHealth` (salud + max). También host
            // para virtuales (subset sin sesión Bedrock).
            $this->addAction('set_health', [
                'target_player' => $this->name,
                'health' => $this->health,
                'max_health' => $this->maxHealth,
            ]);
            $this->hostUpdateHealthFallback();
        }

        public function getMaxHealth(): float
        {
            return $this->maxHealth;
        }

        public function setMaxHealth(float $health): void
        {
            $this->maxHealth = max(1.0, $health);
            if ($this->health > $this->maxHealth) {
                $this->health = $this->maxHealth;
            }
            $this->addAction('set_health', [
                'target_player' => $this->name,
                'health' => $this->health,
                'max_health' => $this->maxHealth,
            ]);
            $this->hostUpdateHealthFallback();
        }

        public function isFlying(): bool
        {
            return $this->flying;
        }

        public function setFlying(bool $flying): void
        {
            $this->flying = $flying;
        }

        public function getXp(): float
        {
            return $this->xp;
        }

        public function setXp(float $xp): void
        {
            $this->xp = max(0.0, $xp);
            // Fase A1: `PluginAction::SetXp` (progress `xp` + `level`).
            // TODO: picks de PM (`Living::setXp` puntos -> progreso/nivel por
            // `ExperienceManager`) no existen; aquí `xp` es progreso 0..1 y
            // `level` el nivel. Conversión puntos->progreso pendiente.
            $this->addAction('set_xp', [
                'target_player' => $this->name,
                'xp' => $this->xp,
                'level' => $this->xpLevel,
            ]);
        }

        public function getXpLevel(): int
        {
            return $this->xpLevel;
        }

        public function setXpLevel(int $level): void
        {
            $this->xpLevel = max(0, $level);
            $this->addAction('set_xp', [
                'target_player' => $this->name,
                'xp' => $this->xp,
                'level' => $this->xpLevel,
            ]);
        }

        public function getInventory(): PlayerInventory
        {
            return $this->inventory;
        }

        public function isOnline(): bool
        {
            return $this->online;
        }

        public function isAlive(): bool
        {
            return $this->health > 0.0;
        }

        public function isClosed(): bool
        {
            return !$this->online;
        }

        public function isConnected(): bool
        {
            return $this->online;
        }

        public function isOp(): bool
        {
            if ($this->opOverride !== null) {
                return $this->opOverride;
            }
            return \pocketmine\Server::getInstance()->isOp($this->name);
        }

        /**
         * PM `Player::hasOpPermission(): bool`. BetterChat (`ChatListener`)
         * lo usa para el prefijo/sufijo de OP; es un alias de `isOp()`.
         */
        public function hasOpPermission(): bool
        {
            return $this->isOp();
        }

        /**
         * Carga perezosa de `first_played`/`last_played` y de la existencia
         * de datos guardados (`player.load` / `player.has_data`) vía host.
         * Sin host, los valores vienen del constructor (0.0 = nunca).
         */
        private function loadPlayedTimes(): void
        {
            if ($this->playedLoaded) {
                return;
            }
            $this->playedLoaded = true;
            if (!class_exists(\pocketmine\runtime\Bridge::class) || !\pocketmine\runtime\Bridge::isActive()) {
                $this->hadSavedData ??= false;
                return;
            }
            try {
                $loaded = \pocketmine\runtime\Bridge::hostCall('player.load', ['name' => $this->name]);
            } catch (\Throwable $e) {
                fwrite(STDERR, '[PHP/PLAYER] player.load failed: ' . $e->getMessage() . PHP_EOL);
                $loaded = null;
            }
            if (!is_array($loaded)) {
                $this->hadSavedData ??= false;
                return;
            }
            $hasData = ($loaded['has_data'] ?? $loaded['exists'] ?? false) === true;
            $this->hadSavedData = $hasData || ($this->hadSavedData ?? false);
            if (!$hasData) {
                return;
            }
            $source = $loaded;
            if (isset($source['data']) && is_array($source['data'])) {
                $source = array_merge($source, $source['data']);
            }
            $first = (float)($source['first_played'] ?? $source['firstPlayed'] ?? 0.0);
            $last = (float)($source['last_played'] ?? $source['lastPlayed'] ?? 0.0);
            if ($first > 0.0) {
                $this->firstPlayed = $first;
            }
            if ($last > 0.0) {
                $this->lastPlayed = $last;
            }
        }

        /** PM `Player::getFirstPlayed(): float` (microtiempo, 0.0 = nunca). */
        public function getFirstPlayed(): float
        {
            $this->loadPlayedTimes();
            return $this->firstPlayed;
        }

        /** PM `Player::getLastPlayed(): float` (microtiempo, 0.0 = nunca). */
        public function getLastPlayed(): float
        {
            $this->loadPlayedTimes();
            return $this->lastPlayed;
        }

        /**
         * PM `Player::hasPlayedBefore(): bool` (umbral de 1ms como axolotl):
         * existían datos guardados antes de esta sesión, o `lastPlayed`
         * supera a `firstPlayed` en más de 1 milisegundo.
         */
        public function hasPlayedBefore(): bool
        {
            $this->loadPlayedTimes();
            return ($this->hadSavedData ?? false) || ($this->lastPlayed > $this->firstPlayed + 0.001);
        }

        public function isPlayer(): bool
        {
            return true;
        }

        public function hasPermission($permission): bool
        {
            if (is_string($permission)) {
                if (array_key_exists($permission, $this->permissions)) {
                    return (bool)$this->permissions[$permission];
                }
                $manager = PermissionManager::getInstance();
                return $manager->hasPermission($manager->getPermission($permission), $this->isOp());
            }
            if ($permission instanceof Permission) {
                return PermissionManager::getInstance()->hasPermission($permission, $this->isOp());
            }
            return false;
        }

        public function sendMessage(string|\pocketmine\lang\Translatable $message): void
        {
            $this->addAction('send_message', ['target_player' => $this->name, 'message' => (string)$message]);
        }

        public function sendTip(string $message): void
        {
            $this->addAction('send_tip', ['target_player' => $this->name, 'message' => $message]);
        }

        public function sendPopup(string $message): void
        {
            $this->addAction('send_popup', ['target_player' => $this->name, 'message' => $message]);
        }

        public function sendTitle(string $title, string $subtitle = '', int $fadeIn = 0, int $stay = 0, int $fadeOut = 0): void
        {
            $this->addAction('send_title', [
                'target_player' => $this->name,
                'title' => $title,
                'subtitle' => $subtitle,
                'fade_in' => $fadeIn,
                'stay' => $stay,
                'fade_out' => $fadeOut,
            ]);
        }

        /** PM `Player::sendSubTitle(string): void` (solo subtítulo). */
        public function sendSubTitle(string $subtitle): void
        {
            $this->addAction('send_subtitle', [
                'target_player' => $this->name,
                'subtitle' => $subtitle,
            ]);
        }

        /** PM `Player::sendActionBarMessage(string): void` (NO es popup). */
        public function sendActionBarMessage(string $message): void
        {
            $this->addAction('send_action_bar', [
                'target_player' => $this->name,
                'message' => $message,
            ]);
        }

        /**
         * PM `Player::sendToastNotification(string, string): void`.
         * Stub documentado: sin paquete Toast en bridge/protocolo.
         * TODO(A1): `ToastNotificationPacket` + queue (maestro Bridge).
         */
        public function sendToastNotification(string $title, string $body): void
        {
            // No-op stub: no se emite acción (variante inexistente rompería IPC).
        }

        /**
         * PM `Player::sendForm(Form): void`. Stub documentado: sin
         * `ModalForm`/`FormResponse` en bridge/protocol.
         * TODO(A1): `ModalFormDataPacket` + `Form` (equipo Forms).
         */
        public function sendForm(mixed $form): void
        {
            // No-op stub: no se emite acción.
        }

        public function teleport(mixed $position): bool
        {
            if ($position instanceof Vector3) {
                $this->x = $position->getX();
                $this->y = $position->getY();
                $this->z = $position->getZ();
                $world = method_exists($position, 'getWorldName') ? $position->getWorldName() : null;
                if (is_string($world) && $world !== '') {
                    $this->worldName = $world;
                }
                if (property_exists($position, 'yaw')) {
                    $this->yaw = (float)$position->yaw;
                }
                if (property_exists($position, 'pitch')) {
                    $this->pitch = (float)$position->pitch;
                }
            }
            $this->addAction('teleport', [
                'target_player' => $this->name,
                'x' => $this->x,
                'y' => $this->y,
                'z' => $this->z,
                'world' => $this->worldName,
            ]);
            return true;
        }

        public function kick(string $reason = '', mixed $quitMessage = null): void
        {
            $this->online = false;
            $this->addAction('kick_player', ['target_player' => $this->name, 'reason' => $reason]);
        }

        public function getServer(): mixed
        {
            return \pocketmine\Server::getInstance();
        }

        public function getUniqueId(): int|string
        {
            return $this->uniqueId;
        }

        public function getXuid(): string
        {
            return (string)$this->uniqueId;
        }

        public function getRuntimeId(): int
        {
            return (int)$this->runtimeId;
        }

        /** @return array<int, string> */
        public function getUsedChunks(): array
        {
            return [];
        }

        // ----------------------------------------------------------------
        // Fase A1: paridad Player.php (visibilidad / skin / inventories /
        // gamemode helpers / toggles / sleep / eat / block-attack stubs).
        // ----------------------------------------------------------------

        /**
         * PM `Player::canSee(Player): bool` (`hiddenPlayers` por UUID).
         * Clave local por nombre (TODO: UUID bytes cuando exista `Uuid` real).
         */
        public function canSee(Player $player): bool
        {
            if ($player === $this) {
                return true;
            }
            return !isset($this->hiddenPlayers[$player->getLowerCaseName()]);
        }

        /**
         * PM `Player::hidePlayer(Player): void` -> `PluginAction::HidePlayer`
         * -> `SessionManager::hide_player` (despawn + flag). No-op si self.
         */
        public function hidePlayer(Player $player): void
        {
            if ($player === $this) {
                return;
            }
            $this->hiddenPlayers[$player->getLowerCaseName()] = true;
            $this->addAction('hide_player', ['viewer' => $this->name, 'target' => $player->getName()]);
        }

        /** PM `Player::showPlayer(Player): void` -> `PluginAction::ShowPlayer`. */
        public function showPlayer(Player $player): void
        {
            if ($player === $this) {
                return;
            }
            unset($this->hiddenPlayers[$player->getLowerCaseName()]);
            $this->addAction('show_player', ['viewer' => $this->name, 'target' => $player->getName()]);
        }

        /**
         * PM `Player::getSkin(): Skin`. Stub Steve (sin textura real).
         * TODO(A1): skin real (128x128 / geometría) + `changeSkin`/`sendSkin`.
         */
        public function getSkin(): object
        {
            if (class_exists(\pocketmine\entity\Skin::class)) {
                return new \pocketmine\entity\Skin();
            }
            $skin = new \stdClass();
            $skin->skinId = 'standard';
            $skin->skinData = '';
            return $skin;
        }

        /** PM `Player::getLocale(): string` (`en_US` por defecto). */
        public function getLocale(): string
        {
            return $this->locale;
        }

        /**
         * PM `Player::getEnderInventory(): EnderInventory`. Stub `PlayerInventory`.
         * TODO(A1): tipo `EnderInventory` + sync con sesión (inventario Rust).
         */
        public function getEnderInventory(): PlayerInventory
        {
            return $this->enderInventory;
        }

        /**
         * PM `Player::getCursorInventory(): PlayerCursorInventory`. Stub.
         * TODO(A1): inventario temporal cursor + sync de ventana.
         */
        public function getCursorInventory(): PlayerInventory
        {
            return $this->cursorInventory;
        }

        /**
         * PM `Player::getCraftingGrid(): CraftingGrid`. Stub.
         * TODO(A1): grid 2x2+output + recetas (CraftingManager).
         */
        public function getCraftingGrid(): PlayerInventory
        {
            return $this->craftingGrid;
        }

        /**
         * PM `Player::getCreativeInventory(): CreativeInventory`. Stub.
         * TODO(A1): inventario creativo compartido + `syncCreative`.
         */
        public function getCreativeInventory(): PlayerInventory
        {
            return $this->creativeInventory;
        }

        /** PM `Player::isSurvival(bool $literal = false)`. */
        public function isSurvival(bool $literal = false): bool
        {
            return $this->gamemode === 0 || (!$literal && $this->gamemode === 2);
        }

        /** PM `Player::isCreative(bool $literal = false)`. */
        public function isCreative(bool $literal = false): bool
        {
            return $this->gamemode === 1 || (!$literal && $this->gamemode === 3);
        }

        /** PM `Player::isAdventure(bool $literal = false)`. */
        public function isAdventure(bool $literal = false): bool
        {
            return $this->gamemode === 2 || (!$literal && $this->gamemode === 3);
        }

        /** PM `Player::isSpectator(): bool`. */
        public function isSpectator(): bool
        {
            return $this->gamemode === 3;
        }

        public function isSprinting(): bool
        {
            return $this->sprinting;
        }

        public function isSneaking(): bool
        {
            return $this->sneaking;
        }

        public function isGliding(): bool
        {
            return $this->gliding;
        }

        public function isSwimming(): bool
        {
            return $this->swimming;
        }

        /** PM `Player::toggleSprint(bool): bool`. Estado local; sync TODO. */
        public function toggleSprint(bool $sprint): bool
        {
            if ($sprint === $this->sprinting) {
                return true;
            }
            // TODO(A1): evento `PlayerToggleSprintEvent` + sync de flags
            // (`SetActorData` DATA_FLAGS) vía `PluginAction::SetFlags`.
            $this->sprinting = $sprint;
            return true;
        }

        /** PM `Player::toggleSneak(bool, bool $sneakPressed = true): bool`. */
        public function toggleSneak(bool $sneak, bool $sneakPressed = true): bool
        {
            $this->sneakPressed = $sneakPressed;
            if ($sneak === $this->sneaking) {
                return true;
            }
            // TODO(A1): evento `PlayerToggleSneakEvent` + sync DATA_FLAGS.
            $this->sneaking = $sneak;
            return true;
        }

        /** PM `Player::toggleFlight(bool): bool` (usa `flying`). */
        public function toggleFlight(bool $fly): bool
        {
            if ($fly === $this->flying) {
                return true;
            }
            // TODO(A1): evento `PlayerToggleFlightEvent` + chequeo
            // `allowFlight` (creative/spectator) + `UpdateAbilitiesPacket`.
            $this->flying = $fly;
            return true;
        }

        /** PM `Player::toggleGlide(bool): bool`. */
        public function toggleGlide(bool $glide): bool
        {
            if ($glide === $this->gliding) {
                return true;
            }
            // TODO(A1): evento + sync FLAG_GLIDING.
            $this->gliding = $glide;
            return true;
        }

        /** PM `Player::toggleSwim(bool): bool`. */
        public function toggleSwim(bool $swim): bool
        {
            if ($swim === $this->swimming) {
                return true;
            }
            // TODO(A1): evento + sync FLAG_SWIMMING.
            $this->swimming = $swim;
            return true;
        }

        /** PM `Player::isSleeping(): bool`. */
        public function isSleeping(): bool
        {
            return $this->sleeping !== null;
        }

        /**
         * PM `Player::sleepOn(Vector3): bool`. Stub: solo guarda posición
         * (floor). TODO(A1): bloque `Bed` + `PlayerBedEnterEvent` + spawn.
         */
        public function sleepOn(mixed $pos): bool
        {
            if ($this->sleeping !== null) {
                return false;
            }
            if ($pos instanceof Vector3) {
                $this->sleeping = $pos->floor();
                return true;
            }
            return false;
        }

        /** PM `Player::stopSleep(): void`. */
        public function stopSleep(): void
        {
            $this->sleeping = null;
        }

        /**
         * PM `Player::canEat(): bool` (`isCreative() || HungerManager`).
         * Simplificación: creative o vivo. TODO: hambre real (`food`).
         */
        public function canEat(): bool
        {
            return $this->isCreative() || $this->isAlive();
        }

        /**
         * PM `Player::canBreathe(): bool` (`isCreative() || air > 0`).
         * Simplificación: creative o vivo. TODO: aire real (`air`).
         */
        public function canBreathe(): bool
        {
            return $this->isCreative() || $this->isAlive();
        }

        /**
         * PM `Player::pickBlock(Vector3, bool): bool`.
         * TODO(A1): `World::getBlock` + `PlayerBlockPickEvent` + item pick.
         * Hoy stub devuelve `false` (sin mundo/bloque real en bridge).
         */
        public function pickBlock(mixed $pos, bool $addTileNBT = false): bool
        {
            return false;
        }

        /**
         * PM `Player::attackBlock(Vector3, int): bool` (clic izquierdo).
         * TODO(A1): `PlayerInteractEvent` + `World::useBreakOn` + handler.
         */
        public function attackBlock(mixed $pos, int $face = 0): bool
        {
            return false;
        }

        /**
         * PM `Player::interactBlock(Vector3, int, Vector3): bool`.
         * TODO(A1): `World::useItemOn` + alcance (`canInteract`).
         */
        public function interactBlock(mixed $pos, int $face = 0, mixed $clickOffset = null): bool
        {
            return false;
        }

        /**
         * PM `Player::attackEntity(Entity): bool`.
         * TODO(A1): `EntityDamageByEntityEvent` + `Entity::attack()` (sin
         * sistema de entidades en bridge).
         */
        public function attackEntity(mixed $entity): bool
        {
            return false;
        }

        /** Fallback virtual: sincroniza salud en registro host si hay Bridge. */
        private function hostUpdateHealthFallback(): void
        {
            if (class_exists(\pocketmine\runtime\Bridge::class) && \pocketmine\runtime\Bridge::isActive()) {
                \pocketmine\runtime\Bridge::hostCall('player.update_health', [
                    'name' => $this->name,
                    'health' => $this->health,
                    'max_health' => $this->maxHealth,
                ]);
            }
        }

        /**
         * Persiste el jugador (`players/<name>.json` vía host `player.save`).
         *
         * Dispara `PlayerDataSaveEvent` antes de escribir; si un plugin lo
         * cancela no se guarda.
         */
        public function save(bool $isQuit = true): bool
        {
            if (class_exists(\pocketmine\event\player\PlayerDataSaveEvent::class)) {
                try {
                    $manager = \pocketmine\Server::getInstance()->getPluginManager();
                    $event = new \pocketmine\event\player\PlayerDataSaveEvent($this);
                    $manager->dispatchEvent($event);
                    if (method_exists($event, 'isCancelled') && $event->isCancelled()) {
                        return false;
                    }
                } catch (\Throwable $e) {
                    fwrite(STDERR, '[PHP/PLAYER] PlayerDataSaveEvent failed: ' . $e->getMessage() . PHP_EOL);
                    return false;
                }
            }
            if (!class_exists(\pocketmine\runtime\Bridge::class) || !\pocketmine\runtime\Bridge::isActive()) {
                return false;
            }
            // JSON v2 canónico: `y` sin +64 legacy; el host completa lo que
            // falte desde el registro/disco y sella `last_played=now`.
            $result = \pocketmine\runtime\Bridge::hostCall('player.save', [
                'name' => $this->name,
                'xuid' => (string)$this->uniqueId,
                'display_name' => $this->displayName,
                'x' => $this->x,
                'y' => $this->y,
                'z' => $this->z,
                'yaw' => $this->yaw,
                'pitch' => $this->pitch,
                'gamemode' => $this->gamemode,
                'health' => $this->health,
                'max_health' => $this->maxHealth,
                'food' => $this->food,
                'saturation' => $this->saturation,
                'xp' => $this->xp,
                'xp_level' => $this->xpLevel,
                'air' => $this->air,
                'flying' => $this->flying,
                'sneaking' => $this->sneaking,
                'sprinting' => $this->sprinting,
                'swimming' => $this->swimming,
                'gliding' => $this->gliding,
                'world' => $this->worldName,
                'is_op' => $this->isOp(),
                'first_played' => $this->firstPlayed,
                'last_played' => $this->lastPlayed,
            ]);
            return is_array($result) && ($result['ok'] ?? false) === true;
        }

        /** Guarda datos genéricos del jugador (`player.save_data`). */
        public function saveData(array $data): bool
        {
            if (!class_exists(\pocketmine\runtime\Bridge::class) || !\pocketmine\runtime\Bridge::isActive()) {
                return false;
            }
            $result = \pocketmine\runtime\Bridge::hostCall('player.save_data', [
                'name' => $this->name,
                'data' => $data,
            ]);
            return is_array($result) && ($result['ok'] ?? false) === true;
        }

        public function toArray(): array
        {
            return [
                'name' => $this->name,
                'display_name' => $this->displayName,
                'address' => $this->address,
                'port' => $this->port,
                'runtime_id' => $this->runtimeId,
                'unique_id' => $this->uniqueId,
                'ping' => $this->ping,
                'gamemode' => $this->gamemode,
                'x' => $this->x,
                'y' => $this->y,
                'z' => $this->z,
                'yaw' => $this->yaw,
                'pitch' => $this->pitch,
                'world' => $this->worldName,
                'health' => $this->health,
                'max_health' => $this->maxHealth,
                'flying' => $this->flying,
                'online' => $this->online,
                'is_op' => $this->isOp(),
                'first_played' => $this->firstPlayed,
                'last_played' => $this->lastPlayed,
            ];
        }

        private function addAction(string $action, array $data = []): void
        {
            try {
                \pocketmine\Server::getInstance()->addAction($action, $data);
            } catch (\Throwable $e) {
                fwrite(STDERR, '[PHP/PLAYER] Unable to queue ' . $action . ': ' . $e->getMessage() . PHP_EOL);
            }
        }

        public function __toString(): string
        {
            return $this->name;
        }
    }

    class OfflinePlayer
    {
        public function __construct(private string $name, private ?array $data = null)
        {
        }

        public function getName(): string
        {
            return $this->name;
        }

        /** Datos guardados (`player.load`), o null si nunca jugó. */
        public function getData(): ?array
        {
            return $this->data;
        }

        public function getXuid(): string
        {
            return (string)($this->data['unique_id'] ?? $this->name);
        }

        public function isOnline(): bool
        {
            return \pocketmine\Server::getInstance()->getPlayerExact($this->name) !== null;
        }

        public function isOp(): bool
        {
            return \pocketmine\Server::getInstance()->isOp($this->name);
        }

        public function hasOpPermission(): bool
        {
            return $this->isOp();
        }

        public function hasPermission($permission): bool
        {
            return $this->isOp();
        }

        /** PM `OfflinePlayer::getFirstPlayed(): float` (0.0 = nunca). */
        public function getFirstPlayed(): float
        {
            $source = $this->data;
            if (is_array($source) && isset($source['data']) && is_array($source['data'])) {
                $source = array_merge($source, $source['data']);
            }
            return (float)($source['first_played'] ?? $source['firstPlayed'] ?? 0.0);
        }

        /** PM `OfflinePlayer::getLastPlayed(): float` (0.0 = nunca). */
        public function getLastPlayed(): float
        {
            $source = $this->data;
            if (is_array($source) && isset($source['data']) && is_array($source['data'])) {
                $source = array_merge($source, $source['data']);
            }
            return (float)($source['last_played'] ?? $source['lastPlayed'] ?? 0.0);
        }

        public function hasPlayedBefore(): bool
        {
            if ($this->data === null) {
                return false;
            }
            return ($this->getLastPlayed() > $this->getFirstPlayed() + 0.001)
                || ($this->data['has_data'] ?? $this->data['exists'] ?? true) === true;
        }

        public function __toString(): string
        {
            return $this->name;
        }
    }
}

namespace pocketmine\inventory {

    use pocketmine\item\Item;
    use pocketmine\item\ItemFactory;

    class PlayerInventory
    {
        /** @var array<int, Item> */
        private array $items = [];

        private int $hotbarSize = 9;
        private int $size = 36;

        public function __construct(private string $ownerName = '')
        {
        }

        public function getItem(int $index): Item
        {
            return $this->items[$index] ?? ItemFactory::get(0, 0, 0);
        }

        public function setItem(int $index, Item $item): void
        {
            if ($item->isNull()) {
                unset($this->items[$index]);
                return;
            }
            $this->items[$index] = $item;
        }

        public function addItem(Item ...$items): array
        {
            $leftover = [];
            foreach ($items as $item) {
                if ($item->isNull()) {
                    continue;
                }
                $placed = false;
                foreach ($this->items as $index => $existing) {
                    if ($existing->getId() === $item->getId() && $existing->getMeta() === $item->getMeta()) {
                        $existing->setCount($existing->getCount() + $item->getCount());
                        $placed = true;
                        break;
                    }
                }
                if (!$placed) {
                    for ($i = 0; $i < $this->size; $i++) {
                        if (!isset($this->items[$i])) {
                            $this->items[$i] = $item;
                            $placed = true;
                            break;
                        }
                    }
                }
                if (!$placed) {
                    $leftover[] = $item;
                }
            }
            return $leftover;
        }

        public function removeItem(Item ...$items): array
        {
            $leftover = [];
            foreach ($items as $item) {
                $remaining = $item->getCount();
                foreach ($this->items as $index => $existing) {
                    if ($existing->getId() !== $item->getId() || $existing->getMeta() !== $item->getMeta()) {
                        continue;
                    }
                    $take = min($remaining, $existing->getCount());
                    $existing->setCount($existing->getCount() - $take);
                    $remaining -= $take;
                    if ($existing->getCount() <= 0) {
                        unset($this->items[$index]);
                    }
                    if ($remaining <= 0) {
                        break;
                    }
                }
                if ($remaining > 0) {
                    $leftover[] = ItemFactory::get($item->getId(), $item->getMeta(), $remaining);
                }
            }
            return $leftover;
        }

        public function first(Item $item, bool $exact = false): int
        {
            foreach ($this->items as $index => $existing) {
                if ($existing->getId() === $item->getId()
                    && (!$exact || $existing->getMeta() === $item->getMeta())) {
                    return $index;
                }
            }
            return -1;
        }

        public function firstEmpty(): int
        {
            for ($i = 0; $i < $this->size; $i++) {
                if (!isset($this->items[$i])) {
                    return $i;
                }
            }
            return -1;
        }

        /** @return array<int, Item> */
        public function getContents(bool $includeEmpty = false): array
        {
            if ($includeEmpty) {
                $contents = [];
                for ($i = 0; $i < $this->size; $i++) {
                    $contents[$i] = $this->getItem($i);
                }
                return $contents;
            }
            return $this->items;
        }

        /** @param array<int, Item> $items */
        public function setContents(array $items): void
        {
            $this->items = [];
            foreach ($items as $index => $item) {
                if ($item instanceof Item && !$item->isNull()) {
                    $this->items[$index] = $item;
                }
            }
        }

        public function clear(int $index): void
        {
            unset($this->items[$index]);
        }

        public function clearAll(): void
        {
            $this->items = [];
        }

        public function getSize(): int
        {
            return $this->size;
        }

        public function getHotbarSize(): int
        {
            return $this->hotbarSize;
        }

        public function getItemInHand(): Item
        {
            return $this->getItem(0);
        }

        public function setItemInHand(Item $item): void
        {
            $this->setItem(0, $item);
        }
    }
}

namespace pocketmine\entity {

    /**
     * PM `pocketmine\entity\Skin` stub (Fase A1): skin Steve por defecto.
     * TODO(A1): textura 64x64/128x128 real + geometría + capa + `setSkin`.
     */
    class Skin
    {
        public function __construct(
            private string $skinId = 'standard',
            private string $skinData = '',
            private string $capeData = '',
            private string $geometryData = ''
        ) {
        }

        public function getSkinId(): string
        {
            return $this->skinId;
        }

        public function getSkinData(): string
        {
            return $this->skinData;
        }

        public function getCapeData(): string
        {
            return $this->capeData;
        }

        public function getGeometryData(): string
        {
            return $this->geometryData;
        }

        public function __toString(): string
        {
            return 'Skin(' . $this->skinId . ')';
        }
    }
}
