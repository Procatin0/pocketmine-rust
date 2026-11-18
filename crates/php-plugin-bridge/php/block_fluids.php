<?php

declare(strict_types=1);

namespace pocketmine\block {

	use pocketmine\block\utils\Ageable;
	use pocketmine\block\utils\AgeableTrait;
	use pocketmine\block\utils\BlockEventHelper;
	use pocketmine\block\utils\HorizontalConnectable;
	use pocketmine\block\utils\MinimumCostFlowCalculator;
	use pocketmine\block\utils\SupportType;
	use pocketmine\data\runtime\RuntimeDataDescriber;
	use pocketmine\entity\Entity;
	use pocketmine\entity\Living;
	use pocketmine\entity\projectile\Arrow;
	use pocketmine\event\block\BlockSpreadEvent;
	use pocketmine\event\entity\EntityCombustByBlockEvent;
	use pocketmine\event\entity\EntityDamageByBlockEvent;
	use pocketmine\event\entity\EntityDamageEvent;
	use pocketmine\item\Item;
	use pocketmine\item\VanillaItems;
	use pocketmine\math\Facing;
	use pocketmine\math\Vector3;
	use pocketmine\utils\Utils;
	use pocketmine\world\format\Chunk;
	use pocketmine\world\sound\FizzSound;
	use pocketmine\world\sound\Sound;
	use pocketmine\world\World;
	use function intdiv;
	use function max;
	use function min;
	use function mt_rand;

	if (!\class_exists(Transparent::class, false)) {
		/**
		 * Transparent blocks do not block any light from propagating through them.
		 */
		class Transparent extends Block {
			public function isTransparent() : bool {
				return true;
			}
		}
	}

	if (!\class_exists(Flowable::class, false)) {
		/**
		 * "Flowable" blocks are destroyed if water flows into the same space as the block. These blocks usually don't have any
		 * collision boxes, and can't provide support for other blocks.
		 */
		abstract class Flowable extends Transparent {

			public function canBeFlowedInto() : bool {
				return true;
			}

			public function isSolid() : bool {
				return false;
			}

			public function canBePlacedAt(Block $blockReplace, Vector3 $clickVector, int $face, bool $isClickedBlock) : bool {
				return (!$this->canBeFlowedInto() || !$blockReplace instanceof Liquid) &&
					parent::canBePlacedAt($blockReplace, $clickVector, $face, $isClickedBlock);
			}

			protected function recalculateCollisionBoxes() : array {
				return [];
			}

			public function getSupportType(int $facing) : SupportType {
				return SupportType::NONE;
			}
		}
	}

	if (!\class_exists(Air::class, false)) {
		/**
		 * Air block
		 */
		class Air extends Flowable {

			public function canBeReplaced() : bool {
				return true;
			}

			public function canBePlaced() : bool {
				return false;
			}
		}
	}

	/**
	 * Liquid block base class (Water, Lava).
	 */
	abstract class Liquid extends Flowable {
		public const MAX_DECAY = 7;

		public int $adjacentSources = 0;

		protected ?Vector3 $flowVector = null;

		protected bool $falling = false;
		protected int $decay = 0; // PC "level" property
		protected bool $still = false;

		protected function describeBlockOnlyState(RuntimeDataDescriber $w) : void {
			if (\method_exists($w, 'boundedIntAuto')) {
				$w->boundedIntAuto(0, self::MAX_DECAY, $this->decay);
			}
			$w->bool($this->falling);
			$w->bool($this->still);
		}

		public function isFalling() : bool { return $this->falling; }

		/** @return $this */
		public function setFalling(bool $falling) : self {
			$this->falling = $falling;
			return $this;
		}

		public function getDecay() : int { return $this->decay; }

		/** @return $this */
		public function setDecay(int $decay) : self {
			if ($decay < 0 || $decay > self::MAX_DECAY) {
				throw new \InvalidArgumentException("Decay must be in range 0 ... " . self::MAX_DECAY);
			}
			$this->decay = $decay;
			return $this;
		}

		public function hasEntityCollision() : bool {
			return true;
		}

		public function canBeReplaced() : bool {
			return true;
		}

		public function canBeFlowedInto() : bool {
			return true;
		}

		public function isSolid() : bool {
			return false;
		}

		protected function recalculateCollisionBoxes() : array {
			return [];
		}

		public function getSupportType(int $facing) : SupportType {
			return SupportType::NONE;
		}

		public function getDropsForCompatibleTool(Item $item) : array {
			return [];
		}

		public function getStillForm() : Block {
			$b = clone $this;
			$b->still = true;
			return $b;
		}

		public function getFlowingForm() : Block {
			$b = clone $this;
			$b->still = false;
			return $b;
		}

		abstract public function getBucketFillSound() : Sound;

		abstract public function getBucketEmptySound() : Sound;

		public function isSource() : bool {
			return !$this->falling && $this->decay === 0;
		}

		/**
		 * @return float
		 */
		public function getFluidHeightPercent() {
			return (($this->falling ? 0 : $this->decay) + 1) / 9;
		}

		public function isStill() : bool {
			return $this->still;
		}

		/** @return $this */
		public function setStill(bool $still = true) : self {
			$this->still = $still;
			return $this;
		}

		protected function getEffectiveFlowDecay(Block $block) : int {
			if (!($block instanceof Liquid) || !$block->hasSameTypeId($this)) {
				return -1;
			}

			return $block->falling ? 0 : $block->decay;
		}

		public function readStateFromWorld() : Block {
			parent::readStateFromWorld();
			$this->flowVector = null;

			return $this;
		}

		public function getFlowVector() : Vector3 {
			if ($this->flowVector !== null) {
				return $this->flowVector;
			}

			$vX = $vY = $vZ = 0;

			$x = $this->position->getFloorX();
			$y = $this->position->getFloorY();
			$z = $this->position->getFloorZ();

			$decay = $this->getEffectiveFlowDecay($this);

			$world = $this->position->getWorld();

			foreach (Facing::HORIZONTAL as $j) {
				[$dx, $dy, $dz] = Facing::OFFSET[$j];

				$sideX = $x + $dx;
				$sideY = $y + $dy;
				$sideZ = $z + $dz;

				$sideBlock = $world->getBlockAt($sideX, $sideY, $sideZ);
				$blockDecay = $this->getEffectiveFlowDecay($sideBlock);

				if ($blockDecay < 0) {
					if (!$sideBlock->canBeFlowedInto()) {
						continue;
					}

					$blockDecay = $this->getEffectiveFlowDecay($world->getBlockAt($sideX, $sideY - 1, $sideZ));

					if ($blockDecay >= 0) {
						$realDecay = $blockDecay - ($decay - 8);
						$vX += $dx * $realDecay;
						$vY += $dy * $realDecay;
						$vZ += $dz * $realDecay;
					}

					continue;
				} else {
					$realDecay = $blockDecay - $decay;
					$vX += $dx * $realDecay;
					$vY += $dy * $realDecay;
					$vZ += $dz * $realDecay;
				}
			}

			$vector = new Vector3($vX, $vY, $vZ);

			if ($this->falling) {
				foreach (Facing::HORIZONTAL as $facing) {
					[$dx, $dy, $dz] = Facing::OFFSET[$facing];
					if (
						!$this->canFlowInto($world->getBlockAt($x + $dx, $y + $dy, $z + $dz)) ||
						!$this->canFlowInto($world->getBlockAt($x + $dx, $y + $dy + 1, $z + $dz))
					) {
						$vector = $vector->normalize()->add(0, -6, 0);
						break;
					}
				}
			}

			return $this->flowVector = $vector->normalize();
		}

		public function addVelocityToEntity(Entity $entity) : ?Vector3 {
			if (\method_exists($entity, 'canBeMovedByCurrents') && $entity->canBeMovedByCurrents()) {
				return $this->getFlowVector();
			}
			return null;
		}

		abstract public function tickRate() : int;

		public function getFlowDecayPerBlock() : int {
			return 1;
		}

		public function getMinAdjacentSourcesToFormSource() : ?int {
			return null;
		}

		public function onNearbyBlockChange() : void {
			if (!$this->checkForHarden()) {
				$this->position->getWorld()->scheduleDelayedBlockUpdate($this->position, $this->tickRate());
			}
		}

		public function onScheduledUpdate() : void {
			$multiplier = $this->getFlowDecayPerBlock();

			$world = $this->position->getWorld();

			$x = $this->position->getFloorX();
			$y = $this->position->getFloorY();
			$z = $this->position->getFloorZ();

			if (!$this->isSource()) {
				$smallestFlowDecay = -100;
				$this->adjacentSources = 0;
				$smallestFlowDecay = $this->getSmallestFlowDecay($world->getBlockAt($x, $y, $z - 1), $smallestFlowDecay);
				$smallestFlowDecay = $this->getSmallestFlowDecay($world->getBlockAt($x, $y, $z + 1), $smallestFlowDecay);
				$smallestFlowDecay = $this->getSmallestFlowDecay($world->getBlockAt($x - 1, $y, $z), $smallestFlowDecay);
				$smallestFlowDecay = $this->getSmallestFlowDecay($world->getBlockAt($x + 1, $y, $z), $smallestFlowDecay);

				$newDecay = $smallestFlowDecay + $multiplier;
				$falling = false;

				if ($newDecay > self::MAX_DECAY || $smallestFlowDecay < 0) {
					$newDecay = -1;
				}

				if ($this->getEffectiveFlowDecay($world->getBlockAt($x, $y + 1, $z)) >= 0) {
					$falling = true;
				}

				$minAdjacentSources = $this->getMinAdjacentSourcesToFormSource();
				if ($minAdjacentSources !== null && $this->adjacentSources >= $minAdjacentSources) {
					$bottomBlock = $world->getBlockAt($x, $y - 1, $z);
					if ($bottomBlock->isSolid() || ($bottomBlock instanceof Liquid && $bottomBlock->hasSameTypeId($this) && $bottomBlock->isSource())) {
						$newDecay = 0;
						$falling = false;
					}
				}

				if ($falling !== $this->falling || (!$falling && $newDecay !== $this->decay)) {
					if (!$falling && $newDecay < 0) {
						$world->setBlockAt($x, $y, $z, VanillaBlocks::AIR());
						return;
					}

					$this->falling = $falling;
					$this->decay = $falling ? 0 : $newDecay;
					$world->setBlockAt($x, $y, $z, $this);
				}
			}

			$bottomBlock = $world->getBlockAt($x, $y - 1, $z);

			$this->flowIntoBlock($bottomBlock, 0, true);

			if ($this->isSource() || !$bottomBlock->canBeFlowedInto()) {
				if ($this->falling) {
					$adjacentDecay = 1;
				} else {
					$adjacentDecay = $this->decay + $multiplier;
				}

				if ($adjacentDecay <= self::MAX_DECAY) {
					$calculator = new MinimumCostFlowCalculator($world, $this->getFlowDecayPerBlock(), $this->canFlowInto(...));
					foreach ($calculator->getOptimalFlowDirections($x, $y, $z) as $facing) {
						[$dx, $dy, $dz] = Facing::OFFSET[$facing];
						$this->flowIntoBlock($world->getBlockAt($x + $dx, $y + $dy, $z + $dz), $adjacentDecay, false);
					}
				}
			}

			$this->checkForHarden();
		}

		protected function flowIntoBlock(Block $block, int $newFlowDecay, bool $falling) : void {
			if ($this->canFlowInto($block) && !($block instanceof Liquid)) {
				$new = clone $this;
				$new->falling = $falling;
				$new->decay = $falling ? 0 : $newFlowDecay;

				$ev = new BlockSpreadEvent($block, $this, $new);
				$ev->call();
				if (!$ev->isCancelled()) {
					$world = $this->position->getWorld();
					if ($block->getTypeId() !== BlockTypeIds::AIR) {
						$world->useBreakOn($block->position);
					}

					$world->setBlock($block->position, $ev->getNewState());
				}
			}
		}

		private function getSmallestFlowDecay(Block $block, int $decay) : int {
			if (!($block instanceof Liquid) || !$block->hasSameTypeId($this)) {
				return $decay;
			}

			$blockDecay = $block->decay;

			if ($block->isSource()) {
				++$this->adjacentSources;
			} elseif ($block->falling) {
				$blockDecay = 0;
			}

			return ($decay >= 0 && $blockDecay >= $decay) ? $decay : $blockDecay;
		}

		protected function checkForHarden() : bool {
			return false;
		}

		protected function liquidCollide(Block $cause, Block $result) : bool {
			if (BlockEventHelper::form($this, $result, $cause)) {
				if (\class_exists(FizzSound::class)) {
					$randomFloat = \class_exists(Utils::class) && \method_exists(Utils::class, 'getRandomFloat')
						? (Utils::getRandomFloat() - Utils::getRandomFloat()) * 0.8
						: 0.0;
					$this->position->getWorld()->addSound($this->position->add(0.5, 0.5, 0.5), new FizzSound(2.6 + $randomFloat));
				}
			}
			return true;
		}

		protected function canFlowInto(Block $block) : bool {
			return
				$this->position->getWorld()->isInWorld($block->position->x, $block->position->y, $block->position->z) &&
				$block->canBeFlowedInto() &&
				!($block instanceof Liquid && $block->isSource());
		}
	}

	/**
	 * Water liquid block.
	 */
	class Water extends Liquid {

		public function getLightFilter() : int {
			return 2;
		}

		public function getBucketFillSound() : Sound {
			if (\class_exists(\pocketmine\world\sound\BucketFillWaterSound::class)) {
				return new \pocketmine\world\sound\BucketFillWaterSound();
			}
			return new class implements Sound {
				public function encode(Vector3 $pos) : array { return []; }
			};
		}

		public function getBucketEmptySound() : Sound {
			if (\class_exists(\pocketmine\world\sound\BucketEmptyWaterSound::class)) {
				return new \pocketmine\world\sound\BucketEmptyWaterSound();
			}
			return new class implements Sound {
				public function encode(Vector3 $pos) : array { return []; }
			};
		}

		public function tickRate() : int {
			return 5;
		}

		public function getMinAdjacentSourcesToFormSource() : ?int {
			return 2;
		}

		public function onEntityInside(Entity $entity) : bool {
			$entity->resetFallDistance();
			if ($entity->isOnFire()) {
				$cause = \defined('\pocketmine\event\entity\EntityExtinguishEvent::CAUSE_WATER')
					? \pocketmine\event\entity\EntityExtinguishEvent::CAUSE_WATER
					: 0;
				$entity->extinguish($cause);
			}
			return true;
		}
	}

	/**
	 * Lava liquid block.
	 */
	class Lava extends Liquid {

		public function getLightLevel() : int {
			return 15;
		}

		public function getBucketFillSound() : Sound {
			if (\class_exists(\pocketmine\world\sound\BucketFillLavaSound::class)) {
				return new \pocketmine\world\sound\BucketFillLavaSound();
			}
			return new class implements Sound {
				public function encode(Vector3 $pos) : array { return []; }
			};
		}

		public function getBucketEmptySound() : Sound {
			if (\class_exists(\pocketmine\world\sound\BucketEmptyLavaSound::class)) {
				return new \pocketmine\world\sound\BucketEmptyLavaSound();
			}
			return new class implements Sound {
				public function encode(Vector3 $pos) : array { return []; }
			};
		}

		public function tickRate() : int {
			return 30;
		}

		public function getFlowDecayPerBlock() : int {
			return 2;
		}

		/**
		 * @phpstan-return \Generator<int, Block, void, void>
		 */
		private function getAdjacentBlocksExceptDown() : \Generator {
			foreach (Facing::ALL as $side) {
				if ($side === Facing::DOWN) {
					continue;
				}
				yield $this->getSide($side);
			}
		}

		protected function checkForHarden() : bool {
			if ($this->falling) {
				return false;
			}
			foreach ($this->getAdjacentBlocksExceptDown() as $colliding) {
				if ($colliding instanceof Water) {
					if ($this->decay === 0) {
						$this->liquidCollide($colliding, VanillaBlocks::OBSIDIAN());
						return true;
					} elseif ($this->decay <= 4) {
						$this->liquidCollide($colliding, VanillaBlocks::COBBLESTONE());
						return true;
					}
				}
			}

			if ($this->getSide(Facing::DOWN)->getTypeId() === BlockTypeIds::SOUL_SOIL) {
				foreach ($this->getAdjacentBlocksExceptDown() as $colliding) {
					if ($colliding->getTypeId() === BlockTypeIds::BLUE_ICE) {
						$this->liquidCollide($colliding, VanillaBlocks::BASALT());
						return true;
					}
				}
			}

			return false;
		}

		protected function flowIntoBlock(Block $block, int $newFlowDecay, bool $falling) : void {
			if ($block instanceof Water) {
				$block->liquidCollide($this, VanillaBlocks::STONE());
			} else {
				parent::flowIntoBlock($block, $newFlowDecay, $falling);
			}
		}

		public function onEntityInside(Entity $entity) : bool {
			$ev = new EntityDamageByBlockEvent($this, $entity, EntityDamageEvent::CAUSE_LAVA, 4);
			$entity->attack($ev);

			$ev = new EntityCombustByBlockEvent($this, $entity, 8);
			$ev->call();
			if (!$ev->isCancelled()) {
				$entity->setOnFire($ev->getDuration());
			}

			$entity->resetFallDistance();
			return true;
		}
	}

	/**
	 * Base fire block.
	 */
	abstract class BaseFire extends Flowable {

		public function hasEntityCollision() : bool {
			return true;
		}

		public function canBeReplaced() : bool {
			return true;
		}

		public function onEntityInside(Entity $entity) : bool {
			$ev = new EntityDamageByBlockEvent($this, $entity, EntityDamageEvent::CAUSE_FIRE, $this->getFireDamage());
			$entity->attack($ev);

			$ev = new EntityCombustByBlockEvent($this, $entity, 8);
			if ($entity instanceof Arrow) {
				$ev->cancel();
			}
			$ev->call();
			if (!$ev->isCancelled()) {
				$entity->setOnFire($ev->getDuration());
			}
			return true;
		}

		abstract protected function getFireDamage() : int;

		public function getDropsForCompatibleTool(Item $item) : array {
			return [];
		}
	}

	/**
	 * Regular fire block.
	 */
	class Fire extends BaseFire implements Ageable, HorizontalConnectable {
		use AgeableTrait;

		public const MAX_AGE = 15;

		private const CONNECTION_FACES = [Facing::UP, Facing::NORTH, Facing::SOUTH, Facing::WEST, Facing::EAST];

		/** @var int[] facing => facing */
		protected array $connections = [];
		protected bool $connectedUp = false;

		protected function describeBlockOnlyState(RuntimeDataDescriber $w) : void {
			if (\method_exists($w, 'boundedIntAuto')) {
				$w->boundedIntAuto(0, self::MAX_AGE, $this->age);
			}
			if (\method_exists($w, 'horizontalFacingFlags')) {
				$w->horizontalFacingFlags($this->connections);
			}
			$w->bool($this->connectedUp);
		}

		protected function getFireDamage() : int {
			return 1;
		}

		public function isConnectedAt(int $facing) : bool {
			return isset($this->connections[$facing]) || ($facing === Facing::UP && $this->connectedUp);
		}

		public function setConnectedAt(int $facing, bool $connected) : void {
			Facing::validate($facing);
			if ($facing === Facing::DOWN) {
				throw new \InvalidArgumentException("DOWN is not a valid facing for this block");
			}
			if ($facing === Facing::UP) {
				$this->connectedUp = $connected;
			} elseif ($connected) {
				$this->connections[$facing] = $facing;
			} else {
				unset($this->connections[$facing]);
			}
		}

		private function canBeSupportedBy(Block $block) : bool {
			return $block->getSupportType(Facing::UP) === SupportType::FULL;
		}

		private function recalculateConnections() : bool {
			$changed = false;
			foreach (self::CONNECTION_FACES as $facing) {
				$connected = $this->getSide($facing)->isFlammable();
				if ($connected !== $this->isConnectedAt($facing)) {
					$this->setConnectedAt($facing, $connected);
					$changed = true;
				}
			}
			return $changed;
		}

		public function onNearbyBlockChange() : void {
			$world = $this->position->getWorld();
			$down = $this->getSide(Facing::DOWN);
			if (SoulFire::canBeSupportedBy($down)) {
				$world->setBlock($this->position, VanillaBlocks::SOUL_FIRE());
			} elseif (!$this->canBeSupportedBy($this->getSide(Facing::DOWN)) && !$this->hasAdjacentFlammableBlocks()) {
				$world->setBlock($this->position, VanillaBlocks::AIR());
			} else {
				if ($this->recalculateConnections()) {
					$world->setBlock($this->position, $this);
				}
				$world->scheduleDelayedBlockUpdate($this->position, mt_rand(30, 40));
			}
		}

		public function ticksRandomly() : bool {
			return true;
		}

		public function onRandomTick() : void {
			$down = $this->getSide(Facing::DOWN);

			$result = null;
			if ($this->age < self::MAX_AGE && mt_rand(0, 2) === 0) {
				$this->age++;
				$result = $this;
			}
			$canSpread = true;

			if (!$down->burnsForever()) {
				if ($this->age === self::MAX_AGE) {
					if (!$down->isFlammable() && mt_rand(0, 3) === 3) {
						$canSpread = false;
						$result = VanillaBlocks::AIR();
					}
				} elseif (!$this->hasAdjacentFlammableBlocks()) {
					$canSpread = false;
					if ($down->isTransparent() || $this->age > 3) {
						$result = VanillaBlocks::AIR();
					}
				}
			}

			$world = $this->position->getWorld();
			if ($result !== null) {
				$world->setBlock($this->position, $result);
			}

			$world->scheduleDelayedBlockUpdate($this->position, mt_rand(30, 40));

			if ($canSpread) {
				$this->burnBlocksAround();
				$this->spreadFire();
			}
		}

		public function onScheduledUpdate() : void {
			$this->onRandomTick();
		}

		private function hasAdjacentFlammableBlocks() : bool {
			foreach (Facing::ALL as $face) {
				if ($this->getSide($face)->isFlammable()) {
					return true;
				}
			}

			return false;
		}

		private function burnBlocksAround() : void {
			foreach ($this->getHorizontalSides() as $side) {
				$this->burnBlock($side, 300);
			}

			$this->burnBlock($this->getSide(Facing::UP), 350);
			$this->burnBlock($this->getSide(Facing::DOWN), 350);
		}

		private function burnBlock(Block $block, int $chanceBound) : void {
			if (mt_rand(0, $chanceBound) < $block->getFlammability()) {
				$cancelled = false;
				if (\class_exists(\pocketmine\event\block\BlockBurnEvent::class) && \pocketmine\event\block\BlockBurnEvent::hasHandlers()) {
					$ev = new \pocketmine\event\block\BlockBurnEvent($block, $this);
					$ev->call();
					$cancelled = $ev->isCancelled();
				}
				if (!$cancelled) {
					$block->onIncinerate();

					$world = $this->position->getWorld();
					if ($world->getBlock($block->position)->isSameState($block)) {
						$spreadedFire = false;
						if (mt_rand(0, $this->age + 9) < 5) {
							$spreadedFire = $this->spreadBlock($block, $this->createSpreadState());
						}
						if (!$spreadedFire) {
							$world->setBlock($block->position, VanillaBlocks::AIR());
						}
					}
				}
			}
		}

		private function spreadFire() : void {
			$world = $this->position->getWorld();
			$difficulty = \method_exists($world, 'getDifficulty') ? $world->getDifficulty() : 1;
			$difficultyChanceIncrease = $difficulty * 7;
			$ageDivisor = $this->age + 30;

			$yMin = \defined('\pocketmine\world\World::Y_MIN') ? World::Y_MIN : -64;
			$yMax = \defined('\pocketmine\world\World::Y_MAX') ? World::Y_MAX : 320;

			for ($y = -1; $y <= 4; ++$y) {
				$targetY = $y + (int) $this->position->y;
				if ($targetY < $yMin || $targetY >= $yMax) {
					continue;
				}
				$randomBound = 100 + ($y > 1 ? ($y - 1) * 100 : 0);

				for ($z = -1; $z <= 1; ++$z) {
					$targetZ = $z + (int) $this->position->z;
					for ($x = -1; $x <= 1; ++$x) {
						if ($x === 0 && $y === 0 && $z === 0) {
							continue;
						}
						$targetX = $x + (int) $this->position->x;
						if (!$world->isInWorld($targetX, $targetY, $targetZ)) {
							continue;
						}

						if (\defined('\pocketmine\world\format\Chunk::COORD_BIT_SIZE')) {
							$chunkX = $targetX >> Chunk::COORD_BIT_SIZE;
							$chunkZ = $targetZ >> Chunk::COORD_BIT_SIZE;
							if (\method_exists($world, 'isChunkLoaded') && !$world->isChunkLoaded($chunkX, $chunkZ)) {
								continue;
							}
						}

						$block = $world->getBlockAt($targetX, $targetY, $targetZ);
						if ($block->getTypeId() !== BlockTypeIds::AIR) {
							continue;
						}

						$encouragement = 0;
						foreach ($block->position->sides() as $vector3) {
							if ($world->isInWorld($vector3->x, $vector3->y, $vector3->z)) {
								$encouragement = max($encouragement, $world->getBlockAt($vector3->x, $vector3->y, $vector3->z)->getFlameEncouragement());
							}
						}

						if ($encouragement <= 0) {
							continue;
						}

						$maxChance = intdiv($encouragement + 40 + $difficultyChanceIncrease, $ageDivisor);

						if ($maxChance > 0 && mt_rand(0, $randomBound - 1) <= $maxChance) {
							$this->spreadBlock($block, $this->createSpreadState());
						}
					}
				}
			}
		}

		private function createSpreadState() : Fire {
			$fire = clone $this;
			$fire->age = min(self::MAX_AGE, $this->age + (mt_rand(0, 4) >> 2));
			$fire->connections = [];
			$fire->connectedUp = false;
			return $fire;
		}

		private function spreadBlock(Block $block, Block $newState) : bool {
			return BlockEventHelper::spread($block, $newState, $this);
		}
	}

	/**
	 * Soul fire block.
	 */
	final class SoulFire extends BaseFire {

		public function getLightLevel() : int {
			return 10;
		}

		protected function getFireDamage() : int {
			return 2;
		}

		public static function canBeSupportedBy(Block $block) : bool {
			$id = $block->getTypeId();
			return $id === BlockTypeIds::SOUL_SAND || $id === BlockTypeIds::SOUL_SOIL;
		}

		public function onNearbyBlockChange() : void {
			if (!self::canBeSupportedBy($this->getSide(Facing::DOWN))) {
				$this->position->getWorld()->setBlock($this->position, VanillaBlocks::AIR());
			}
		}
	}

	/**
	 * Cobweb block.
	 */
	class Cobweb extends Flowable {

		public function hasEntityCollision() : bool {
			return true;
		}

		public function onEntityInside(Entity $entity) : bool {
			$entity->resetFallDistance();
			return true;
		}

		public function getDropsForCompatibleTool(Item $item) : array {
			if (($item->getBlockToolType() & BlockToolType::SHEARS) !== 0) {
				return [$this->asItem()];
			}
			if (\class_exists(VanillaItems::class)) {
				return [
					VanillaItems::STRING()
				];
			}
			return [];
		}

		public function isAffectedBySilkTouch() : bool {
			return true;
		}

		public function blocksDirectSkyLight() : bool {
			return true;
		}
	}

	/**
	 * Magma block.
	 */
	class Magma extends Opaque {

		public function getLightLevel() : int {
			return 3;
		}

		public function hasEntityCollision() : bool {
			return true;
		}

		public function onEntityInside(Entity $entity) : bool {
			if ($entity instanceof Living && !$entity->isSneaking() && (!\method_exists($entity, 'getFrostWalkerLevel') || $entity->getFrostWalkerLevel() === 0)) {
				$ev = new EntityDamageByBlockEvent($this, $entity, EntityDamageEvent::CAUSE_FIRE, 1);
				$entity->attack($ev);
			}
			return true;
		}

		public function burnsForever() : bool {
			return true;
		}
	}
}
