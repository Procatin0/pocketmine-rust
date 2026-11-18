<?php

declare(strict_types=1);

namespace pocketmine\world\generator\object {

	use pocketmine\utils\LegacyEnumShimTrait;

	if (!\enum_exists(TreeType::class, false)) {
		enum TreeType {
			use LegacyEnumShimTrait;

			case OAK;
			case SPRUCE;
			case BIRCH;
			case JUNGLE;
			case ACACIA;
			case DARK_OAK;
			case CRIMSON;
			case WARPED;
			case AZALEA;
			case MANGROVE;
			case CHERRY;
			case PALE_OAK;

			public function getDisplayName() : string {
				return match($this) {
					self::OAK => 'Oak',
					self::SPRUCE => 'Spruce',
					self::BIRCH => 'Birch',
					self::JUNGLE => 'Jungle',
					self::ACACIA => 'Acacia',
					self::DARK_OAK => 'Dark Oak',
					self::CRIMSON => 'Crimson',
					self::WARPED => 'Warped',
					self::AZALEA => 'Azalea',
					self::MANGROVE => 'Mangrove',
					self::CHERRY => 'Cherry',
					self::PALE_OAK => 'Pale Oak',
				};
			}
		}
	}
}


namespace pocketmine\block {

	use pocketmine\block\utils\Ageable;
	use pocketmine\block\utils\AgeableTrait;
	use pocketmine\block\utils\BlockEventHelper;
	use pocketmine\block\utils\CropGrowthHelper;
	use pocketmine\block\utils\DripleafState;
	use pocketmine\block\utils\FortuneDropHelper;
	use pocketmine\block\utils\HorizontalConnectable;
	use pocketmine\block\utils\HorizontalFacing;
	use pocketmine\block\utils\HorizontalFacingTrait;
	use pocketmine\block\utils\LeavesType;
	use pocketmine\block\utils\MultiAnyFacing;
	use pocketmine\block\utils\MultiAnySupportTrait;
	use pocketmine\block\utils\MushroomBlockType;
	use pocketmine\block\utils\SaplingType;
	use pocketmine\block\utils\StaticSupportTrait;
	use pocketmine\block\utils\SupportType;
	use pocketmine\block\utils\TallGrassTrait;
	use pocketmine\block\utils\WoodMaterial;
	use pocketmine\block\utils\WoodType;
	use pocketmine\data\runtime\RuntimeDataDescriber;
	use pocketmine\entity\effect\EffectInstance;
	use pocketmine\entity\effect\VanillaEffects;
	use pocketmine\entity\Entity;
	use pocketmine\entity\Living;
	use pocketmine\entity\projectile\Projectile;
	use pocketmine\event\block\LeavesDecayEvent;
	use pocketmine\event\block\StructureGrowEvent;
	use pocketmine\event\entity\EntityDamageByBlockEvent;
	use pocketmine\event\entity\EntityDamageEvent;
	use pocketmine\item\Fertilizer;
	use pocketmine\item\Item;
	use pocketmine\item\VanillaItems;
	use pocketmine\math\Axis;
	use pocketmine\math\AxisAlignedBB;
	use pocketmine\math\Facing;
	use pocketmine\math\RayTraceResult;
	use pocketmine\math\Vector3;
	use pocketmine\player\Player;
	use pocketmine\utils\Random;
	use pocketmine\world\BlockTransaction;
	use pocketmine\world\generator\object\TreeFactory;
	use pocketmine\world\generator\object\TreeType;
	use pocketmine\world\Position;
	use pocketmine\world\World;
	use function array_intersect_key;
	use function array_rand;
	use function count;
	use function min;
	use function mt_rand;

	// ------------------------------------------------------------------------
	// Common Plant base
	// ------------------------------------------------------------------------

	/**
	 * Abstract common base class for plant blocks.
	 */
	abstract class Plant extends Flowable {
	}

	// ------------------------------------------------------------------------
	// Leaves
	// ------------------------------------------------------------------------

	/**
	 * PM Leaves block.
	 */
	class Leaves extends Transparent {
		private const MAX_LOG_DISTANCE = 4;

		protected LeavesType $leavesType;
		protected bool $noDecay = false;
		protected bool $checkDecay = false;

		public function __construct(BlockIdentifier $idInfo, string $name, BlockTypeInfo $typeInfo, ?LeavesType $leavesType = null) {
			parent::__construct($idInfo, $name, $typeInfo);
			$this->leavesType = $leavesType ?? LeavesType::OAK;
		}

		protected function describeBlockOnlyState(RuntimeDataDescriber $w) : void {
			$w->bool($this->noDecay);
			$w->bool($this->checkDecay);
		}

		public function getLeavesType() : LeavesType { return $this->leavesType; }

		public function isNoDecay() : bool { return $this->noDecay; }

		/** @return $this */
		public function setNoDecay(bool $noDecay) : self {
			$this->noDecay = $noDecay;
			return $this;
		}

		public function isCheckDecay() : bool { return $this->checkDecay; }

		/** @return $this */
		public function setCheckDecay(bool $checkDecay) : self {
			$this->checkDecay = $checkDecay;
			return $this;
		}

		public function blocksDirectSkyLight() : bool {
			return true;
		}

		/**
		 * @param true[] $visited reference parameter
		 * @phpstan-param array<int, true> $visited
		 * @phpstan-param-out array<int, true> $visited
		 */
		protected function findLog(Vector3 $pos, array &$visited = [], int $distance = 0) : bool {
			$index = World::blockHash((int)$pos->x, (int)$pos->y, (int)$pos->z);
			if (isset($visited[$index])) {
				return false;
			}
			$visited[$index] = true;

			$block = $this->position->getWorld()->getBlock($pos);
			if ($block instanceof WoodMaterial || \str_contains(\strtolower($block->getName()), 'log') || \str_contains(\strtolower($block->getName()), 'wood')) {
				return true;
			}

			if ($block instanceof Leaves && $distance <= self::MAX_LOG_DISTANCE) {
				foreach (Facing::ALL as $side) {
					if ($this->findLog($pos->getSide($side), $visited, $distance + 1)) {
						return true;
					}
				}
			}

			return false;
		}

		public function onNearbyBlockChange() : void {
			if (!$this->noDecay && !$this->checkDecay) {
				$this->checkDecay = true;
				$this->position->getWorld()->setBlock($this->position, $this, false);
			}
		}

		public function ticksRandomly() : bool {
			return !$this->noDecay && $this->checkDecay;
		}

		public function onRandomTick() : void {
			if (!$this->noDecay && $this->checkDecay) {
				$cancelled = false;
				if (\class_exists(LeavesDecayEvent::class) && LeavesDecayEvent::hasHandlers()) {
					$ev = new LeavesDecayEvent($this);
					$ev->call();
					$cancelled = $ev->isCancelled();
				}

				$world = $this->position->getWorld();
				if ($cancelled || $this->findLog($this->position)) {
					$this->checkDecay = false;
					$world->setBlock($this->position, $this, false);
				} else {
					$world->useBreakOn($this->position);
				}
			}
		}

		public function place(BlockTransaction $tx, Item $item, Block $blockReplace, Block $blockClicked, int $face, Vector3 $clickVector, ?Player $player = null) : bool {
			$this->noDecay = true; // artificial leaves don't decay
			return parent::place($tx, $item, $blockReplace, $blockClicked, $face, $clickVector, $player);
		}

		public function getDropsForCompatibleTool(Item $item) : array {
			if (($item->getBlockToolType() & BlockToolType::SHEARS) !== 0) {
				return parent::getDropsForCompatibleTool($item);
			}

			$drops = [];
			if (FortuneDropHelper::bonusChanceDivisor($item, 20, 4)) {
				$sapling = (match($this->leavesType) {
					LeavesType::ACACIA => VanillaBlocks::ACACIA_SAPLING(),
					LeavesType::BIRCH => VanillaBlocks::BIRCH_SAPLING(),
					LeavesType::DARK_OAK => VanillaBlocks::DARK_OAK_SAPLING(),
					LeavesType::JUNGLE => VanillaBlocks::JUNGLE_SAPLING(),
					LeavesType::OAK => VanillaBlocks::OAK_SAPLING(),
					LeavesType::SPRUCE => VanillaBlocks::SPRUCE_SAPLING(),
					LeavesType::AZALEA => VanillaBlocks::AZALEA(),
					LeavesType::FLOWERING_AZALEA => VanillaBlocks::FLOWERING_AZALEA(),
					default => null,
				})?->asItem();
				if ($sapling !== null) {
					$drops[] = $sapling;
				}
			}
			if (
				($this->leavesType === LeavesType::OAK || $this->leavesType === LeavesType::DARK_OAK) &&
				FortuneDropHelper::bonusChanceDivisor($item, 200, 20)
			) {
				$drops[] = VanillaItems::APPLE();
			}
			if (FortuneDropHelper::bonusChanceDivisor($item, 50, 5)) {
				$drops[] = VanillaItems::STICK()->setCount(mt_rand(1, 2));
			}

			return $drops;
		}

		public function isAffectedBySilkTouch() : bool {
			return true;
		}

		public function getFlameEncouragement() : int {
			return 30;
		}

		public function getFlammability() : int {
			return 60;
		}

		public function getSupportType(int $facing) : SupportType {
			return SupportType::NONE;
		}
	}

	// ------------------------------------------------------------------------
	// Crops & Stem Family
	// ------------------------------------------------------------------------

	/**
	 * Base class for all farm crops.
	 */
	abstract class Crops extends Flowable implements Ageable {
		use AgeableTrait;
		use StaticSupportTrait;

		public const MAX_AGE = 7;

		private function canBeSupportedAt(Block $block) : bool {
			return $block->getSide(Facing::DOWN)->getTypeId() === BlockTypeIds::FARMLAND;
		}

		public function onInteract(Item $item, int $face, Vector3 $clickVector, ?Player $player = null, array &$returnedItems = []) : bool {
			if ($this->age < self::MAX_AGE && ($item instanceof Fertilizer || \str_contains(\strtolower($item->getName()), 'bone meal') || \str_contains(\strtolower($item->getName()), 'fertilizer'))) {
				$block = clone $this;
				$tempAge = $block->age + mt_rand(2, 5);
				if ($tempAge > self::MAX_AGE) {
					$tempAge = self::MAX_AGE;
				}
				$block->age = $tempAge;
				if (BlockEventHelper::grow($this, $block, $player)) {
					$item->pop();
				}

				return true;
			}

			return false;
		}

		public function ticksRandomly() : bool {
			return $this->age < self::MAX_AGE;
		}

		public function onRandomTick() : void {
			if ($this->age < self::MAX_AGE && CropGrowthHelper::canGrow($this)) {
				$block = clone $this;
				++$block->age;
				BlockEventHelper::grow($this, $block, null);
			}
		}
	}

	class Wheat extends Crops {

		public function getDropsForCompatibleTool(Item $item) : array {
			if ($this->age >= self::MAX_AGE) {
				return [
					VanillaItems::WHEAT(),
					VanillaItems::WHEAT_SEEDS()->setCount(FortuneDropHelper::binomial($item, 0))
				];
			}
			return [
				VanillaItems::WHEAT_SEEDS()
			];
		}

		public function asItem() : Item {
			return VanillaItems::WHEAT_SEEDS();
		}
	}

	class Carrot extends Crops {

		public function getDropsForCompatibleTool(Item $item) : array {
			return [
				VanillaItems::CARROT()->setCount($this->age >= self::MAX_AGE ? FortuneDropHelper::binomial($item, 1) : 1)
			];
		}

		public function asItem() : Item {
			return VanillaItems::CARROT();
		}
	}

	class Potato extends Crops {

		public function getDropsForCompatibleTool(Item $item) : array {
			$result = [
				VanillaItems::POTATO()->setCount($this->age >= self::MAX_AGE ? FortuneDropHelper::binomial($item, 1) : 1)
			];
			if ($this->age >= self::MAX_AGE && mt_rand(0, 49) === 0) {
				$result[] = VanillaItems::POISONOUS_POTATO();
			}
			return $result;
		}

		public function asItem() : Item {
			return VanillaItems::POTATO();
		}
	}

	class Beetroot extends Crops {

		public function getDropsForCompatibleTool(Item $item) : array {
			if ($this->age >= self::MAX_AGE) {
				return [
					VanillaItems::BEETROOT(),
					VanillaItems::BEETROOT_SEEDS()->setCount(FortuneDropHelper::binomial($item, 0))
				];
			}

			return [
				VanillaItems::BEETROOT_SEEDS()
			];
		}

		public function asItem() : Item {
			return VanillaItems::BEETROOT_SEEDS();
		}
	}

	abstract class Stem extends Crops {
		protected int $facing = Facing::UP;

		protected function describeBlockOnlyState(RuntimeDataDescriber $w) : void {
			parent::describeBlockOnlyState($w);
			if (\method_exists($w, 'facingExcept')) {
				$w->facingExcept($this->facing, Facing::DOWN);
			}
		}

		public function getFacing() : int { return $this->facing; }

		/** @return $this */
		public function setFacing(int $facing) : self {
			if ($facing === Facing::DOWN) {
				throw new \InvalidArgumentException("DOWN is not a valid facing for this block");
			}
			$this->facing = $facing;
			return $this;
		}

		abstract protected function getPlant() : Block;

		public function onNearbyBlockChange() : void {
			if ($this->facing !== Facing::UP && !$this->getSide($this->facing)->hasSameTypeId($this->getPlant())) {
				$this->position->getWorld()->setBlock($this->position, $this->setFacing(Facing::UP));
			}
			parent::onNearbyBlockChange();
		}

		public function ticksRandomly() : bool {
			return $this->age < self::MAX_AGE || $this->facing === Facing::UP;
		}

		public function onRandomTick() : void {
			if ($this->facing === Facing::UP && CropGrowthHelper::canGrow($this)) {
				$world = $this->position->getWorld();
				if ($this->age < self::MAX_AGE) {
					$block = clone $this;
					++$block->age;
					BlockEventHelper::grow($this, $block, null);
				} else {
					$grow = $this->getPlant();
					foreach (Facing::HORIZONTAL as $side) {
						if ($this->getSide($side)->hasSameTypeId($grow)) {
							return;
						}
					}

					$facing = Facing::HORIZONTAL[array_rand(Facing::HORIZONTAL)];
					$side = $this->getSide($facing);
					if ($side->getTypeId() === BlockTypeIds::AIR && $side->getSide(Facing::DOWN)->hasTypeTag(BlockTypeTags::DIRT)) {
						if (BlockEventHelper::grow($side, $grow, null)) {
							$this->position->getWorld()->setBlock($this->position, $this->setFacing($facing));
						}
					}
				}
			}
		}

		public function getDropsForCompatibleTool(Item $item) : array {
			return [
				$this->asItem()->setCount(FortuneDropHelper::binomial(VanillaItems::AIR(), 0, ($this->age + 1) / 15))
			];
		}
	}

	class MelonStem extends Stem {

		protected function getPlant() : Block {
			return VanillaBlocks::MELON();
		}

		public function asItem() : Item {
			return VanillaItems::MELON_SEEDS();
		}
	}

	class PumpkinStem extends Stem {

		protected function getPlant() : Block {
			return VanillaBlocks::PUMPKIN();
		}

		public function asItem() : Item {
			return VanillaItems::PUMPKIN_SEEDS();
		}
	}

	// ------------------------------------------------------------------------
	// Pitcher Crop & Torchflower Crop
	// ------------------------------------------------------------------------

	final class PitcherCrop extends Flowable implements Ageable {
		use AgeableTrait;
		use StaticSupportTrait;

		public const MAX_AGE = 2;

		private function canBeSupportedAt(Block $block) : bool {
			return $block->getSide(Facing::DOWN)->getTypeId() === BlockTypeIds::FARMLAND;
		}

		public function onInteract(Item $item, int $face, Vector3 $clickVector, ?Player $player = null, array &$returnedItems = []) : bool {
			if ($item instanceof Fertilizer || \str_contains(\strtolower($item->getName()), 'bone meal') || \str_contains(\strtolower($item->getName()), 'fertilizer')) {
				$world = $this->position->getWorld();
				$up = $this->position->up();
				if ($this->age < self::MAX_AGE && $world->isInWorld((int)$up->x, (int)$up->y, (int)$up->z) && $world->getBlock($up)->canBeReplaced()) {
					$tx = new BlockTransaction($world);
					$tx->addBlock($this->position, (clone $this)->setAge($this->age + 1));
					$ev = new StructureGrowEvent($this, $tx, $player);
					$ev->call();
					if (!$ev->isCancelled() && $tx->apply()) {
						$item->pop();
						return true;
					}
				}
			}
			return false;
		}

		public function ticksRandomly() : bool {
			return $this->age < self::MAX_AGE;
		}

		public function onRandomTick() : void {
			if ($this->age < self::MAX_AGE && CropGrowthHelper::canGrow($this)) {
				$block = clone $this;
				++$block->age;
				BlockEventHelper::grow($this, $block, null);
			}
		}

		public function getDropsForCompatibleTool(Item $item) : array {
			return [VanillaItems::PITCHER_POD()];
		}

		public function asItem() : Item {
			return VanillaItems::PITCHER_POD();
		}
	}

	final class TorchflowerCrop extends Flowable {
		use StaticSupportTrait;

		private bool $ready = false;

		public function describeBlockOnlyState(RuntimeDataDescriber $w) : void {
			$w->bool($this->ready);
		}

		public function isReady() : bool { return $this->ready; }

		public function setReady(bool $ready) : self {
			$this->ready = $ready;
			return $this;
		}

		private function canBeSupportedAt(Block $block) : bool {
			return $block->getSide(Facing::DOWN)->getTypeId() === BlockTypeIds::FARMLAND;
		}

		public function onInteract(Item $item, int $face, Vector3 $clickVector, ?Player $player = null, array &$returnedItems = []) : bool {
			if (!$this->ready && ($item instanceof Fertilizer || \str_contains(\strtolower($item->getName()), 'bone meal') || \str_contains(\strtolower($item->getName()), 'fertilizer'))) {
				$block = (clone $this)->setReady(true);
				if (BlockEventHelper::grow($this, $block, $player)) {
					$item->pop();
					return true;
				}
			}
			return false;
		}

		public function ticksRandomly() : bool {
			return !$this->ready;
		}

		public function onRandomTick() : void {
			if (!$this->ready && CropGrowthHelper::canGrow($this)) {
				$block = (clone $this)->setReady(true);
				BlockEventHelper::grow($this, $block, null);
			}
		}

		public function getDropsForCompatibleTool(Item $item) : array {
			if ($this->ready) {
				return [VanillaBlocks::TORCHFLOWER()->asItem()];
			}
			return [VanillaItems::TORCHFLOWER_SEEDS()];
		}

		public function asItem() : Item {
			return VanillaItems::TORCHFLOWER_SEEDS();
		}
	}

	// ------------------------------------------------------------------------
	// Sapling
	// ------------------------------------------------------------------------

	class Sapling extends Flowable {
		use StaticSupportTrait;

		protected bool $ready = false;
		private SaplingType $saplingType;

		public function __construct(BlockIdentifier $idInfo, string $name, BlockTypeInfo $typeInfo, ?SaplingType $saplingType = null) {
			parent::__construct($idInfo, $name, $typeInfo);
			$this->saplingType = $saplingType ?? SaplingType::OAK;
		}

		protected function describeBlockOnlyState(RuntimeDataDescriber $w) : void {
			$w->bool($this->ready);
		}

		public function isReady() : bool { return $this->ready; }

		/** @return $this */
		public function setReady(bool $ready) : self {
			$this->ready = $ready;
			return $this;
		}

		public function getSaplingType() : SaplingType { return $this->saplingType; }

		private function canBeSupportedAt(Block $block) : bool {
			$supportBlock = $block->getSide(Facing::DOWN);
			return $supportBlock->hasTypeTag(BlockTypeTags::DIRT) || $supportBlock->hasTypeTag(BlockTypeTags::MUD);
		}

		public function onInteract(Item $item, int $face, Vector3 $clickVector, ?Player $player = null, array &$returnedItems = []) : bool {
			if (($item instanceof Fertilizer || \str_contains(\strtolower($item->getName()), 'bone meal') || \str_contains(\strtolower($item->getName()), 'fertilizer')) && $this->grow($player)) {
				$item->pop();
				return true;
			}

			return false;
		}

		public function ticksRandomly() : bool {
			return true;
		}

		public function onRandomTick() : void {
			$world = $this->position->getWorld();
			if ($world->getFullLightAt($this->position->getFloorX(), $this->position->getFloorY(), $this->position->getFloorZ()) >= 8 && mt_rand(1, 7) === 1) {
				if ($this->ready) {
					$this->grow(null);
				} else {
					$this->ready = true;
					$world->setBlock($this->position, $this);
				}
			}
		}

		private function grow(?Player $player) : bool {
			if (!\class_exists('pocketmine\world\generator\object\TreeFactory')) {
				return false; // Native generator tree growth fallback
			}
			$random = new Random(mt_rand());
			$tree = TreeFactory::get($random, $this->saplingType->getTreeType());
			$transaction = $tree?->getBlockTransaction($this->position->getWorld(), $this->position->getFloorX(), $this->position->getFloorY(), $this->position->getFloorZ(), $random);
			if ($transaction === null) {
				return false;
			}

			$ev = new StructureGrowEvent($this, $transaction, $player);
			$ev->call();
			if (!$ev->isCancelled()) {
				return $transaction->apply();
			}
			return false;
		}

		public function getFuelTime() : int {
			return 100;
		}
	}

	// ------------------------------------------------------------------------
	// Bamboo & BambooSapling
	// ------------------------------------------------------------------------

	class Bamboo extends Transparent {
		use StaticSupportTrait;

		public const NO_LEAVES = 0;
		public const SMALL_LEAVES = 1;
		public const LARGE_LEAVES = 2;

		protected bool $thick = false;
		protected bool $ready = false;
		protected int $leafSize = self::NO_LEAVES;

		protected function describeBlockOnlyState(RuntimeDataDescriber $w) : void {
			if (\method_exists($w, 'boundedIntAuto')) {
				$w->boundedIntAuto(self::NO_LEAVES, self::LARGE_LEAVES, $this->leafSize);
			}
			$w->bool($this->thick);
			$w->bool($this->ready);
		}

		public function isThick() : bool { return $this->thick; }

		/** @return $this */
		public function setThick(bool $thick) : self {
			$this->thick = $thick;
			return $this;
		}

		public function isReady() : bool { return $this->ready; }

		/** @return $this */
		public function setReady(bool $ready) : self {
			$this->ready = $ready;
			return $this;
		}

		public function getLeafSize() : int { return $this->leafSize; }

		/** @return $this */
		public function setLeafSize(int $leafSize) : self {
			$this->leafSize = $leafSize;
			return $this;
		}

		protected function recalculateCollisionBoxes() : array {
			return [
				AxisAlignedBB::one()
					->trim(Facing::WEST, 0.5)
					->trim(Facing::EAST, 0.5)
					->trim(Facing::NORTH, 0.5)
					->trim(Facing::SOUTH, 0.5)
			];
		}

		public function getSupportType(int $facing) : SupportType {
			return SupportType::NONE;
		}

		private function canBeSupportedAt(Block $block) : bool {
			$down = $block->getSide(Facing::DOWN);
			return $down->hasSameTypeId($this) ||
				$down->hasTypeTag(BlockTypeTags::DIRT) ||
				$down->hasTypeTag(BlockTypeTags::MUD) ||
				$down->hasTypeTag(BlockTypeTags::SAND) ||
				$down->getTypeId() === BlockTypeIds::GRAVEL;
		}

		public function onInteract(Item $item, int $face, Vector3 $clickVector, ?Player $player = null, array &$returnedItems = []) : bool {
			if ($item instanceof Fertilizer || \str_contains(\strtolower($item->getName()), 'bone meal') || \str_contains(\strtolower($item->getName()), 'fertilizer')) {
				$world = $this->position->getWorld();
				$up = $this->position->up();
				if ($world->isInWorld((int)$up->x, (int)$up->y, (int)$up->z) && $world->getBlock($up)->canBeReplaced()) {
					if (BlockEventHelper::grow($world->getBlock($up), VanillaBlocks::BAMBOO(), $player)) {
						$item->pop();
						return true;
					}
				}
			}
			return false;
		}

		public function ticksRandomly() : bool {
			return true;
		}

		public function onRandomTick() : void {
			$world = $this->position->getWorld();
			$up = $this->position->up();
			if ($world->isInWorld((int)$up->x, (int)$up->y, (int)$up->z) && $world->getBlock($up)->canBeReplaced() && mt_rand(1, 3) === 1) {
				BlockEventHelper::grow($world->getBlock($up), VanillaBlocks::BAMBOO(), null);
			}
		}

		public function asItem() : Item {
			return VanillaItems::BAMBOO();
		}
	}

	final class BambooSapling extends Flowable {
		use StaticSupportTrait;

		private bool $ready = false;

		protected function describeBlockOnlyState(RuntimeDataDescriber $w) : void {
			$w->bool($this->ready);
		}

		public function isReady() : bool { return $this->ready; }

		/** @return $this */
		public function setReady(bool $ready) : self {
			$this->ready = $ready;
			return $this;
		}

		private function canBeSupportedAt(Block $block) : bool {
			$supportBlock = $block->getSide(Facing::DOWN);
			return
				$supportBlock->getTypeId() === BlockTypeIds::GRAVEL ||
				$supportBlock->hasTypeTag(BlockTypeTags::DIRT) ||
				$supportBlock->hasTypeTag(BlockTypeTags::MUD) ||
				$supportBlock->hasTypeTag(BlockTypeTags::SAND);
		}

		public function onInteract(Item $item, int $face, Vector3 $clickVector, ?Player $player = null, array &$returnedItems = []) : bool {
			if ($item instanceof Fertilizer || \str_contains(\strtolower($item->getName()), 'bone meal') || \str_contains(\strtolower($item->getName()), 'bamboo')) {
				if ($this->grow($player)) {
					$item->pop();
					return true;
				}
			}
			return false;
		}

		private function grow(?Player $player) : bool {
			$world = $this->position->getWorld();
			if (!$world->getBlock($this->position->up())->canBeReplaced()) {
				return false;
			}

			$tx = new BlockTransaction($world);
			$bamboo = VanillaBlocks::BAMBOO();
			$tx->addBlock($this->position, $bamboo)
				->addBlock($this->position->up(), (clone $bamboo)->setLeafSize(Bamboo::SMALL_LEAVES));

			$ev = new StructureGrowEvent($this, $tx, $player);
			$ev->call();
			if ($ev->isCancelled()) {
				return false;
			}

			return $tx->apply();
		}

		public function ticksRandomly() : bool {
			return true;
		}

		public function onRandomTick() : void {
			$world = $this->position->getWorld();
			if ($this->ready) {
				$this->ready = false;
				if ($world->getFullLight($this->position) < 9 || !$this->grow(null)) {
					$world->setBlock($this->position, $this);
				}
			} elseif ($world->getBlock($this->position->up())->canBeReplaced()) {
				$this->ready = true;
				$world->setBlock($this->position, $this);
			}
		}

		public function asItem() : Item {
			return VanillaItems::BAMBOO();
		}
	}

	// ------------------------------------------------------------------------
	// Cactus & CactusFlower
	// ------------------------------------------------------------------------

	class Cactus extends Transparent implements Ageable {
		use AgeableTrait;
		use StaticSupportTrait;

		public const MAX_AGE = 15;
		public const MAX_HEIGHT = 3;

		public function hasEntityCollision() : bool {
			return true;
		}

		protected function recalculateCollisionBoxes() : array {
			$shrinkSize = 1 / 16;
			return [AxisAlignedBB::one()->contract($shrinkSize, 0, $shrinkSize)->trim(Facing::UP, $shrinkSize)];
		}

		public function getSupportType(int $facing) : SupportType {
			return SupportType::NONE;
		}

		public function onEntityInside(Entity $entity) : bool {
			$ev = new EntityDamageByBlockEvent($this, $entity, EntityDamageEvent::CAUSE_CONTACT, 1);
			$entity->attack($ev);
			return true;
		}

		private function canBeSupportedAt(Block $block) : bool {
			$supportBlock = $block->getSide(Facing::DOWN);
			if (!$supportBlock->hasSameTypeId($this) && !$supportBlock->hasTypeTag(BlockTypeTags::SAND)) {
				return false;
			}
			foreach (Facing::HORIZONTAL as $side) {
				if ($block->getSide($side)->isSolid()) {
					return false;
				}
			}

			return true;
		}

		public function ticksRandomly() : bool {
			return true;
		}

		public function onRandomTick() : void {
			$up = $this->getSide(Facing::UP);
			if ($up->getTypeId() !== BlockTypeIds::AIR) {
				return;
			}

			$world = $this->position->getWorld();

			if (!$world->isInWorld((int)$up->position->x, (int)$up->position->y, (int)$up->position->z)) {
				return;
			}

			$height = 1;
			while ($height < self::MAX_HEIGHT && $this->getSide(Facing::DOWN, $height)->hasSameTypeId($this)) {
				$height++;
			}

			if ($this->age === 9) {
				$canGrowFlower = true;
				foreach (Facing::HORIZONTAL as $side) {
					if ($up->getSide($side)->isSolid()) {
						$canGrowFlower = false;
						break;
					}
				}

				if ($canGrowFlower) {
					$chance = $height >= self::MAX_HEIGHT ? 25 : 10;
					if (mt_rand(1, 100) <= $chance) {
						if (BlockEventHelper::grow($up, VanillaBlocks::CACTUS_FLOWER(), null)) {
							$this->age = 0;
							$world->setBlock($this->position, $this, update: false);
						}
						return;
					}
				}
			}

			if ($this->age === self::MAX_AGE) {
				$this->age = 0;

				if ($height < self::MAX_HEIGHT) {
					BlockEventHelper::grow($up, VanillaBlocks::CACTUS(), null);
				}
			} else {
				++$this->age;
			}
			$world->setBlock($this->position, $this, update: false);
		}
	}

	class CactusFlower extends Flowable {
		use StaticSupportTrait;

		private function canBeSupportedAt(Block $block) : bool {
			$supportBlock = $block->getSide(Facing::DOWN);
			return
				$supportBlock->getSupportType(Facing::UP)->hasCenterSupport() ||
				$supportBlock->getTypeId() === BlockTypeIds::CACTUS;
		}

		public function getFlameEncouragement() : int {
			return 60;
		}

		public function getFlammability() : int {
			return 100;
		}
	}

	// ------------------------------------------------------------------------
	// Sugarcane & NetherWartPlant
	// ------------------------------------------------------------------------

	class Sugarcane extends Flowable implements Ageable {
		use AgeableTrait;
		use StaticSupportTrait {
			onNearbyBlockChange as onSupportBlockChange;
		}

		public const MAX_AGE = 15;

		private function seekToBottom() : Position {
			$world = $this->position->getWorld();
			$bottom = $this->position;
			while (($next = $world->getBlock($bottom->down()))->hasSameTypeId($this)) {
				$bottom = $next->position;
			}
			return $bottom;
		}

		private function grow(Position $pos, ?Player $player = null) : bool {
			$grew = false;
			$world = $pos->getWorld();
			for ($y = 1; $y < 3; ++$y) {
				if (!$world->isInWorld((int)$pos->x, (int)$pos->y + $y, (int)$pos->z)) {
					break;
				}
				$b = $world->getBlockAt((int)$pos->x, (int)$pos->y + $y, (int)$pos->z);
				if ($b->getTypeId() === BlockTypeIds::AIR) {
					if (BlockEventHelper::grow($b, VanillaBlocks::SUGARCANE(), $player)) {
						$grew = true;
					} else {
						break;
					}
				} elseif (!$b->hasSameTypeId($this)) {
					break;
				}
			}
			$this->age = 0;
			$world->setBlock($pos, $this);
			return $grew;
		}

		public function onInteract(Item $item, int $face, Vector3 $clickVector, ?Player $player = null, array &$returnedItems = []) : bool {
			if ($item instanceof Fertilizer || \str_contains(\strtolower($item->getName()), 'bone meal') || \str_contains(\strtolower($item->getName()), 'fertilizer')) {
				if ($this->grow($this->seekToBottom(), $player)) {
					$item->pop();
				}

				return true;
			}

			return false;
		}

		private function canBeSupportedAt(Block $block) : bool {
			$supportBlock = $block->getSide(Facing::DOWN);
			return $supportBlock->hasSameTypeId($this) ||
				$supportBlock->hasTypeTag(BlockTypeTags::MUD) ||
				$supportBlock->hasTypeTag(BlockTypeTags::DIRT) ||
				$supportBlock->hasTypeTag(BlockTypeTags::SAND);
		}

		public function ticksRandomly() : bool {
			return true;
		}

		public function onRandomTick() : void {
			$down = $this->getSide(Facing::DOWN);
			if (!$down->hasSameTypeId($this)) {
				if (!$this->hasNearbyWater($down)) {
					$this->position->getWorld()->useBreakOn($this->position, createParticles: true);
					return;
				}

				if ($this->age === self::MAX_AGE) {
					$this->grow($this->position);
				} else {
					++$this->age;
					$this->position->getWorld()->setBlock($this->position, $this);
				}
			}
		}

		public function place(BlockTransaction $tx, Item $item, Block $blockReplace, Block $blockClicked, int $face, Vector3 $clickVector, ?Player $player = null) : bool {
			$down = $blockReplace->getSide(Facing::DOWN);
			if ($down->hasSameTypeId($this)) {
				return parent::place($tx, $item, $blockReplace, $blockClicked, $face, $clickVector, $player);
			}

			foreach (Facing::HORIZONTAL as $side) {
				$sideBlock = $down->getSide($side);
				if ($sideBlock instanceof Water || $sideBlock->getTypeId() === BlockTypeIds::WATER) {
					return parent::place($tx, $item, $blockReplace, $blockClicked, $face, $clickVector, $player);
				}
			}

			return false;
		}

		private function hasNearbyWater(Block $down) : bool {
			foreach ($down->getHorizontalSides() as $sideBlock) {
				$blockId = $sideBlock->getTypeId();
				if ($blockId === BlockTypeIds::WATER || $sideBlock instanceof Water) {
					return true;
				}
			}
			return false;
		}

		public function onNearbyBlockChange() : void {
			$down = $this->getSide(Facing::DOWN);
			if (!$down->hasSameTypeId($this) && !$this->hasNearbyWater($down)) {
				$this->position->getWorld()->useBreakOn($this->position, createParticles: true);
			} else {
				$this->onSupportBlockChange();
			}
		}
	}

	class NetherWartPlant extends Flowable implements Ageable {
		use AgeableTrait;
		use StaticSupportTrait;

		public const MAX_AGE = 3;

		private function canBeSupportedAt(Block $block) : bool {
			return $block->getSide(Facing::DOWN)->getTypeId() === BlockTypeIds::SOUL_SAND;
		}

		public function ticksRandomly() : bool {
			return $this->age < self::MAX_AGE;
		}

		public function onRandomTick() : void {
			if ($this->age < self::MAX_AGE && mt_rand(0, 10) === 0) {
				$block = clone $this;
				$block->age++;
				BlockEventHelper::grow($this, $block, null);
			}
		}

		public function getDropsForCompatibleTool(Item $item) : array {
			return [
				$this->asItem()->setCount($this->age === self::MAX_AGE ? FortuneDropHelper::discrete($item, 2, 4) : 1)
			];
		}
	}

	// ------------------------------------------------------------------------
	// CocoaBlock
	// ------------------------------------------------------------------------

	class CocoaBlock extends Flowable implements Ageable, HorizontalFacing {
		use HorizontalFacingTrait;
		use AgeableTrait;

		public const MAX_AGE = 2;

		protected function describeBlockOnlyState(RuntimeDataDescriber $w) : void {
			$w->horizontalFacing($this->facing);
			if (\method_exists($w, 'boundedIntAuto')) {
				$w->boundedIntAuto(0, self::MAX_AGE, $this->age);
			}
		}

		protected function recalculateCollisionBoxes() : array {
			return [
				AxisAlignedBB::one()
					->squash(Facing::axis(Facing::rotateY($this->facing, true)), (6 - $this->age) / 16)
					->trim(Facing::DOWN, (7 - $this->age * 2) / 16)
					->trim(Facing::UP, 0.25)
					->trim(Facing::opposite($this->facing), 1 / 16)
					->trim($this->facing, (11 - $this->age * 2) / 16)
			];
		}

		private function canAttachTo(Block $block) : bool {
			return ($block instanceof WoodMaterial && $block->getWoodType() === WoodType::JUNGLE) ||
				$block->getTypeId() === BlockTypeIds::JUNGLE_LOG ||
				$block->getTypeId() === BlockTypeIds::JUNGLE_WOOD;
		}

		public function place(BlockTransaction $tx, Item $item, Block $blockReplace, Block $blockClicked, int $face, Vector3 $clickVector, ?Player $player = null) : bool {
			if (Facing::axis($face) !== Axis::Y && $this->canAttachTo($blockClicked)) {
				$this->facing = $face;
				return parent::place($tx, $item, $blockReplace, $blockClicked, $face, $clickVector, $player);
			}

			return false;
		}

		public function onInteract(Item $item, int $face, Vector3 $clickVector, ?Player $player = null, array &$returnedItems = []) : bool {
			if (($item instanceof Fertilizer || \str_contains(\strtolower($item->getName()), 'bone meal') || \str_contains(\strtolower($item->getName()), 'fertilizer')) && $this->grow($player)) {
				$item->pop();
				return true;
			}

			return false;
		}

		public function onNearbyBlockChange() : void {
			if (!$this->canAttachTo($this->getSide(Facing::opposite($this->facing)))) {
				$this->position->getWorld()->useBreakOn($this->position);
			}
		}

		public function ticksRandomly() : bool {
			return $this->age < self::MAX_AGE;
		}

		public function onRandomTick() : void {
			if (mt_rand(1, 5) === 1) {
				$this->grow();
			}
		}

		private function grow(?Player $player = null) : bool {
			if ($this->age < self::MAX_AGE) {
				$block = clone $this;
				$block->age++;
				return BlockEventHelper::grow($this, $block, $player);
			}
			return false;
		}

		public function getDropsForCompatibleTool(Item $item) : array {
			return [
				VanillaItems::COCOA_BEANS()->setCount($this->age === self::MAX_AGE ? mt_rand(2, 3) : 1)
			];
		}

		public function asItem() : Item {
			return VanillaItems::COCOA_BEANS();
		}
	}

	// ------------------------------------------------------------------------
	// ChorusFlower & ChorusPlant
	// ------------------------------------------------------------------------

	final class ChorusFlower extends Flowable implements Ageable {
		use AgeableTrait;
		use StaticSupportTrait;

		public const MIN_AGE = 0;
		public const MAX_AGE = 5;

		private const MAX_STEM_HEIGHT = 5;

		protected function recalculateCollisionBoxes() : array {
			return [AxisAlignedBB::one()];
		}

		private function canBeSupportedAt(Block $block) : bool {
			$position = $block->position;
			$world = $position->getWorld();
			$down = $world->getBlock($position->down());

			if ($down->getTypeId() === BlockTypeIds::END_STONE || $down->getTypeId() === BlockTypeIds::CHORUS_PLANT) {
				return true;
			}

			$plantAdjacent = false;
			foreach ($position->sidesAroundAxis(Axis::Y) as $sidePosition) {
				$b = $world->getBlock($sidePosition);

				if ($b->getTypeId() === BlockTypeIds::CHORUS_PLANT) {
					if ($plantAdjacent) {
						return false;
					}
					$plantAdjacent = true;
				} elseif ($b->getTypeId() !== BlockTypeIds::AIR) {
					return false;
				}
			}

			return $plantAdjacent;
		}

		public function onProjectileHit(Projectile $projectile, RayTraceResult $hitResult) : void {
			$this->position->getWorld()->useBreakOn($this->position);
		}

		/**
		 * @phpstan-return array{int, bool}
		 */
		private function scanStem() : array {
			$world = $this->position->getWorld();

			$stemHeight = 0;
			$endStoneBelow = false;
			for ($yOffset = 0; $yOffset < self::MAX_STEM_HEIGHT; $yOffset++, $stemHeight++) {
				$down = $world->getBlock($this->position->down($yOffset + 1));

				if ($down->getTypeId() !== BlockTypeIds::CHORUS_PLANT) {
					if ($down->getTypeId() === BlockTypeIds::END_STONE) {
						$endStoneBelow = true;
					}
					break;
				}
			}

			return [$stemHeight, $endStoneBelow];
		}

		private function allHorizontalBlocksEmpty(World $world, Vector3 $position, ?int $except) : bool {
			foreach ($position->sidesAroundAxis(Axis::Y) as $facing => $sidePosition) {
				if ($facing === $except) {
					continue;
				}
				if ($world->getBlock($sidePosition)->getTypeId() !== BlockTypeIds::AIR) {
					return false;
				}
			}

			return true;
		}

		private function canGrowUpwards(int $stemHeight, bool $endStoneBelow) : bool {
			$world = $this->position->getWorld();

			$up = $this->position->up();
			if (
				!$world->isInWorld((int)$up->x, (int)$up->y, (int)$up->z) ||
				$world->getBlock($up)->getTypeId() !== BlockTypeIds::AIR ||
				(
					$world->isInWorld((int)$up->x, (int)$up->y + 1, (int)$up->z) &&
					$world->getBlock($up->up())->getTypeId() !== BlockTypeIds::AIR
				)
			) {
				return false;
			}

			if ($this->getSide(Facing::DOWN)->getTypeId() !== BlockTypeIds::AIR) {
				if ($stemHeight >= self::MAX_STEM_HEIGHT) {
					return false;
				}

				if ($stemHeight > 1 && $stemHeight > mt_rand(0, $endStoneBelow ? 4 : 3)) {
					return false;
				}
			}

			return $this->allHorizontalBlocksEmpty($world, $up, null);
		}

		private function grow(int $facing, int $ageChange, ?BlockTransaction $tx) : BlockTransaction {
			if ($tx === null) {
				$tx = new BlockTransaction($this->position->getWorld());
			}
			$tx->addBlock($this->position->getSide($facing), (clone $this)->setAge(min(self::MAX_AGE, $this->age + $ageChange)));

			return $tx;
		}

		public function ticksRandomly() : bool { return $this->age < self::MAX_AGE; }

		public function onRandomTick() : void {
			$world = $this->position->getWorld();

			if ($this->age >= self::MAX_AGE) {
				return;
			}

			$tx = null;

			[$stemHeight, $endStoneBelow] = $this->scanStem();
			if ($this->canGrowUpwards($stemHeight, $endStoneBelow)) {
				$tx = $this->grow(Facing::UP, 0, $tx);
			} else {
				$facingVisited = [];
				for ($attempts = 0, $maxAttempts = mt_rand(0, $endStoneBelow ? 4 : 3); $attempts < $maxAttempts; $attempts++) {
					$facing = Facing::HORIZONTAL[array_rand(Facing::HORIZONTAL)];
					if (isset($facingVisited[$facing])) {
						continue;
					}
					$facingVisited[$facing] = true;

					$sidePosition = $this->position->getSide($facing);
					if (
						$world->getBlock($sidePosition)->getTypeId() === BlockTypeIds::AIR &&
						$world->getBlock($sidePosition->down())->getTypeId() === BlockTypeIds::AIR &&
						$this->allHorizontalBlocksEmpty($world, $sidePosition, Facing::opposite($facing))
					) {
						$tx = $this->grow($facing, 1, $tx);
					}
				}
			}

			if ($tx !== null) {
				$tx->addBlock($this->position, VanillaBlocks::CHORUS_PLANT());
				$ev = new StructureGrowEvent($this, $tx, null);
				$ev->call();
				if (!$ev->isCancelled() && $tx->apply()) {
					if (\class_exists('pocketmine\world\sound\ChorusFlowerGrowSound')) {
						$world->addSound($this->position->add(0.5, 0.5, 0.5), new \pocketmine\world\sound\ChorusFlowerGrowSound());
					}
				}
			} else {
				if (\class_exists('pocketmine\world\sound\ChorusFlowerDieSound')) {
					$world->addSound($this->position->add(0.5, 0.5, 0.5), new \pocketmine\world\sound\ChorusFlowerDieSound());
				}
				$this->position->getWorld()->setBlock($this->position, $this->setAge(self::MAX_AGE));
			}
		}
	}

	final class ChorusPlant extends Flowable implements HorizontalConnectable {
		use StaticSupportTrait;

		/** @var int[] facing => facing */
		protected array $connections = [];

		protected function describeBlockOnlyState(RuntimeDataDescriber $w) : void {
			if (\method_exists($w, 'facingFlags')) {
				$w->facingFlags($this->connections);
			}
		}

		public function isConnectedAt(int $facing) : bool {
			return isset($this->connections[$facing]);
		}

		public function setConnectedAt(int $facing, bool $connected) : void {
			Facing::validate($facing);
			if ($connected) {
				$this->connections[$facing] = $facing;
			} else {
				unset($this->connections[$facing]);
			}
		}

		protected function recalculateCollisionBoxes() : array {
			$bb = AxisAlignedBB::one();
			foreach (Facing::ALL as $facing) {
				if (!isset($this->connections[$facing])) {
					$bb->trim($facing, 2 / 16);
				}
			}

			return [$bb];
		}

		private function recalculateConnections() : bool {
			$changed = false;
			foreach (Facing::ALL as $facing) {
				$block = $this->getSide($facing);
				$connected = match($block->getTypeId()) {
					BlockTypeIds::CHORUS_FLOWER, $this->getTypeId() => true,
					BlockTypeIds::END_STONE => $facing === Facing::DOWN,
					default => false
				};
				if ($connected !== $this->isConnectedAt($facing)) {
					$this->setConnectedAt($facing, $connected);
					$changed = true;
				}
			}
			return $changed;
		}

		public function onNearbyBlockChange() : void {
			if (!$this->canBeSupportedAt($this)) {
				$this->position->getWorld()->useBreakOn($this->position);
			} elseif ($this->recalculateConnections()) {
				$this->position->getWorld()->setBlock($this->position, $this);
			}
		}

		private function canBeSupportedBy(Block $block) : bool {
			return $block->hasSameTypeId($this) || $block->getTypeId() === BlockTypeIds::END_STONE;
		}

		private function canBeSupportedAt(Block $block) : bool {
			$position = $block->position;
			$world = $position->getWorld();

			$down = $world->getBlock($position->down());
			$verticalAir = $down->getTypeId() === BlockTypeIds::AIR || $world->getBlock($position->up())->getTypeId() === BlockTypeIds::AIR;

			foreach ($position->sidesAroundAxis(Axis::Y) as $sidePosition) {
				$b = $world->getBlock($sidePosition);

				if ($b->getTypeId() === BlockTypeIds::CHORUS_PLANT) {
					if (!$verticalAir) {
						return false;
					}

					if ($this->canBeSupportedBy($b->getSide(Facing::DOWN))) {
						return true;
					}
				}
			}

			return $this->canBeSupportedBy($down);
		}

		public function getDropsForCompatibleTool(Item $item) : array {
			if (mt_rand(0, 1) === 1) {
				return [VanillaItems::CHORUS_FRUIT()];
			}

			return [];
		}
	}

	// ------------------------------------------------------------------------
	// Vines, GlowLichen, SeaPickle
	// ------------------------------------------------------------------------

	class Vine extends Flowable {

		/** @var int[] */
		protected array $faces = [];

		protected function describeBlockOnlyState(RuntimeDataDescriber $w) : void {
			if (\method_exists($w, 'horizontalFacingFlags')) {
				$w->horizontalFacingFlags($this->faces);
			}
		}

		/** @return int[] */
		public function getFaces() : array { return $this->faces; }

		public function hasFace(int $face) : bool {
			return isset($this->faces[$face]);
		}

		/**
		 * @param int[] $faces
		 * @return $this
		 */
		public function setFaces(array $faces) : self {
			$uniqueFaces = [];
			foreach ($faces as $face) {
				if ($face !== Facing::NORTH && $face !== Facing::SOUTH && $face !== Facing::WEST && $face !== Facing::EAST) {
					throw new \InvalidArgumentException("Facing can only be north, east, south or west");
				}
				$uniqueFaces[$face] = $face;
			}
			$this->faces = $uniqueFaces;
			return $this;
		}

		/** @return $this */
		public function setFace(int $face, bool $value) : self {
			if ($face !== Facing::NORTH && $face !== Facing::SOUTH && $face !== Facing::WEST && $face !== Facing::EAST) {
				throw new \InvalidArgumentException("Facing can only be north, east, south or west");
			}
			if ($value) {
				$this->faces[$face] = $face;
			} else {
				unset($this->faces[$face]);
			}
			return $this;
		}

		public function hasEntityCollision() : bool {
			return true;
		}

		public function canClimb() : bool {
			return true;
		}

		public function canBeReplaced() : bool {
			return true;
		}

		public function onEntityInside(Entity $entity) : bool {
			$entity->resetFallDistance();
			return true;
		}

		protected function recalculateCollisionBoxes() : array {
			return [];
		}

		public function place(BlockTransaction $tx, Item $item, Block $blockReplace, Block $blockClicked, int $face, Vector3 $clickVector, ?Player $player = null) : bool {
			if (!$blockReplace->getSide(Facing::opposite($face))->isFullCube() || Facing::axis($face) === Axis::Y) {
				return false;
			}

			$this->faces = $blockReplace instanceof Vine ? $blockReplace->faces : [];
			$this->faces[Facing::opposite($face)] = Facing::opposite($face);

			return parent::place($tx, $item, $blockReplace, $blockClicked, $face, $clickVector, $player);
		}

		public function onNearbyBlockChange() : void {
			$changed = false;

			$up = $this->getSide(Facing::UP);
			$supportedFaces = $up instanceof Vine ? array_intersect_key($this->faces, $up->faces) : [];

			foreach ($this->faces as $face) {
				if (!isset($supportedFaces[$face]) && !$this->getSide($face)->isSolid()) {
					unset($this->faces[$face]);
					$changed = true;
				}
			}

			if ($changed) {
				$world = $this->position->getWorld();
				if (count($this->faces) === 0) {
					$world->useBreakOn($this->position);
				} else {
					$world->setBlock($this->position, $this);
				}
			}
		}

		public function ticksRandomly() : bool {
			return true;
		}

		public function onRandomTick() : void {
			// Stub: vine growth handled safely
		}

		public function getDrops(mixed $item = null) : array {
			if ($item instanceof Item && ($item->getBlockToolType() & BlockToolType::SHEARS) !== 0) {
				return $this->getDropsForCompatibleTool($item);
			}

			return [];
		}

		public function getFlameEncouragement() : int {
			return 15;
		}

		public function getFlammability() : int {
			return 100;
		}
	}

	class GlowLichen extends Transparent implements MultiAnyFacing {
		use MultiAnySupportTrait;

		public function getLightLevel() : int {
			return 7;
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

		public function canBeReplaced() : bool {
			return true;
		}

		/** @return int[] */
		protected function getInitialPlaceFaces(Block $blockReplace) : array {
			return $blockReplace instanceof GlowLichen ? $blockReplace->faces : [];
		}

		public function onInteract(Item $item, int $face, Vector3 $clickVector, ?Player $player = null, array &$returnedItems = []) : bool {
			if ($item instanceof Fertilizer || \str_contains(\strtolower($item->getName()), 'bone meal') || \str_contains(\strtolower($item->getName()), 'fertilizer')) {
				$item->pop();
				return true;
			}
			return false;
		}

		public function getDrops(mixed $item = null) : array {
			if ($item instanceof Item && ($item->getBlockToolType() & BlockToolType::SHEARS) !== 0) {
				return [$this->asItem()];
			}
			return [];
		}
	}

	class SeaPickle extends Transparent {
		public const MIN_COUNT = 1;
		public const MAX_COUNT = 4;

		protected int $count = self::MIN_COUNT;
		protected bool $underwater = false;

		protected function describeBlockOnlyState(RuntimeDataDescriber $w) : void {
			if (\method_exists($w, 'boundedIntAuto')) {
				$w->boundedIntAuto(self::MIN_COUNT, self::MAX_COUNT, $this->count);
			}
			$w->bool($this->underwater);
		}

		public function getCount() : int { return $this->count; }

		/** @return $this */
		public function setCount(int $count) : self {
			if ($count < self::MIN_COUNT || $count > self::MAX_COUNT) {
				throw new \InvalidArgumentException("Count must be in range " . self::MIN_COUNT . " ... " . self::MAX_COUNT);
			}
			$this->count = $count;
			return $this;
		}

		public function isUnderwater() : bool { return $this->underwater; }

		/** @return $this */
		public function setUnderwater(bool $underwater) : self {
			$this->underwater = $underwater;
			return $this;
		}

		public function isSolid() : bool {
			return false;
		}

		public function getLightLevel() : int {
			return $this->underwater ? ($this->count + 1) * 3 : 0;
		}

		protected function recalculateCollisionBoxes() : array {
			return [];
		}

		public function getSupportType(int $facing) : SupportType {
			return SupportType::NONE;
		}

		public function canBePlacedAt(Block $blockReplace, Vector3 $clickVector, int $face, bool $isClickedBlock) : bool {
			return ($blockReplace instanceof SeaPickle && $blockReplace->count < self::MAX_COUNT) || parent::canBePlacedAt($blockReplace, $clickVector, $face, $isClickedBlock);
		}

		public function place(BlockTransaction $tx, Item $item, Block $blockReplace, Block $blockClicked, int $face, Vector3 $clickVector, ?Player $player = null) : bool {
			$this->underwater = false;
			if ($blockReplace instanceof SeaPickle && $blockReplace->count < self::MAX_COUNT) {
				$this->count = $blockReplace->count + 1;
			}

			return parent::place($tx, $item, $blockReplace, $blockClicked, $face, $clickVector, $player);
		}

		public function getDropsForCompatibleTool(Item $item) : array {
			return [$this->asItem()->setCount($this->count)];
		}
	}

	// ------------------------------------------------------------------------
	// Dripleaf Family
	// ------------------------------------------------------------------------

	abstract class BaseBigDripleaf extends Transparent implements HorizontalFacing {
		use HorizontalFacingTrait;

		abstract protected function isHead() : bool;

		private function canBeSupportedBy(Block $block, bool $head) : bool {
			return
				($block instanceof BaseBigDripleaf && $block->isHead() === $head) ||
				$block->getTypeId() === BlockTypeIds::CLAY ||
				$block->hasTypeTag(BlockTypeTags::DIRT) ||
				$block->hasTypeTag(BlockTypeTags::MUD);
		}

		public function onNearbyBlockChange() : void {
			if (
				(!$this->isHead() && !$this->getSide(Facing::UP) instanceof BaseBigDripleaf) ||
				!$this->canBeSupportedBy($this->getSide(Facing::DOWN), false)
			) {
				$this->position->getWorld()->useBreakOn($this->position);
			}
		}

		public function place(BlockTransaction $tx, Item $item, Block $blockReplace, Block $blockClicked, int $face, Vector3 $clickVector, ?Player $player = null) : bool {
			$block = $blockReplace->getSide(Facing::DOWN);
			if (!$this->canBeSupportedBy($block, true)) {
				return false;
			}
			if ($player !== null) {
				$this->facing = Facing::opposite($player->getHorizontalFacing());
			}
			if ($block instanceof BaseBigDripleaf) {
				$this->facing = $block->facing;
				$tx->addBlock($block->position, VanillaBlocks::BIG_DRIPLEAF_STEM()->setFacing($this->facing));
			}
			return parent::place($tx, $item, $blockReplace, $blockClicked, $face, $clickVector, $player);
		}

		public function onInteract(Item $item, int $face, Vector3 $clickVector, ?Player $player = null, array &$returnedItems = []) : bool {
			if (($item instanceof Fertilizer || \str_contains(\strtolower($item->getName()), 'bone meal') || \str_contains(\strtolower($item->getName()), 'fertilizer')) && $this->grow($player)) {
				$item->pop();
				return true;
			}
			return false;
		}

		private function seekToHead() : ?BaseBigDripleaf {
			if ($this->isHead()) {
				return $this;
			}
			$step = 1;
			while (($next = $this->getSide(Facing::UP, $step)) instanceof BaseBigDripleaf) {
				if ($next->isHead()) {
					return $next;
				}
				$step++;
			}
			return null;
		}

		private function grow(?Player $player) : bool {
			$head = $this->seekToHead();
			if ($head === null) {
				return false;
			}
			$pos = $head->position;
			$up = $pos->up();
			$world = $pos->getWorld();
			if (
				!$world->isInWorld($up->getFloorX(), $up->getFloorY(), $up->getFloorZ()) ||
				$world->getBlock($up)->getTypeId() !== BlockTypeIds::AIR
			) {
				return false;
			}

			$tx = new BlockTransaction($world);
			$tx->addBlock($pos, VanillaBlocks::BIG_DRIPLEAF_STEM()->setFacing($head->facing));
			$tx->addBlock($up, VanillaBlocks::BIG_DRIPLEAF_HEAD()->setFacing($head->facing));

			$ev = new StructureGrowEvent($head, $tx, $player);
			$ev->call();

			if (!$ev->isCancelled()) {
				return $tx->apply();
			}
			return false;
		}

		public function getFlameEncouragement() : int {
			return 15;
		}

		public function getFlammability() : int {
			return 100;
		}

		public function getSupportType(int $facing) : SupportType {
			return SupportType::NONE;
		}
	}

	class BigDripleafHead extends BaseBigDripleaf {
		protected DripleafState $leafState = DripleafState::STABLE;

		protected function describeBlockOnlyState(RuntimeDataDescriber $w) : void {
			parent::describeBlockOnlyState($w);
			if (\method_exists($w, 'enum')) {
				$w->enum($this->leafState);
			}
		}

		protected function isHead() : bool {
			return true;
		}

		public function getLeafState() : DripleafState {
			return $this->leafState;
		}

		/** @return $this */
		public function setLeafState(DripleafState $leafState) : self {
			$this->leafState = $leafState;
			return $this;
		}

		public function hasEntityCollision() : bool {
			return true;
		}

		private function setTiltAndScheduleTick(DripleafState $tilt) : void {
			$this->position->getWorld()->setBlock($this->position, $this->setLeafState($tilt));
			$delay = $tilt->getScheduledUpdateDelayTicks();
			if ($delay !== null) {
				$this->position->getWorld()->scheduleDelayedBlockUpdate($this->position, $delay);
			}
		}

		private function getLeafTopOffset() : float {
			return match($this->leafState) {
				DripleafState::STABLE, DripleafState::UNSTABLE => 1 / 16,
				DripleafState::PARTIAL_TILT => 3 / 16,
				default => 0.0,
			};
		}

		public function onEntityInside(Entity $entity) : bool {
			if (!$entity instanceof Projectile && $this->leafState === DripleafState::STABLE) {
				$intersection = AxisAlignedBB::one()
					->offset($this->position->x, $this->position->y, $this->position->z)
					->trim(Facing::DOWN, 1 - $this->getLeafTopOffset());
				if ($entity->getBoundingBox()->intersectsWith($intersection)) {
					$this->setTiltAndScheduleTick(DripleafState::UNSTABLE);
					return false;
				}
			}
			return true;
		}

		public function onProjectileHit(Projectile $projectile, RayTraceResult $hitResult) : void {
			if ($this->leafState !== DripleafState::FULL_TILT) {
				$this->setTiltAndScheduleTick(DripleafState::FULL_TILT);
				if (\class_exists('pocketmine\world\sound\DripleafTiltDownSound')) {
					$this->position->getWorld()->addSound($this->position, new \pocketmine\world\sound\DripleafTiltDownSound());
				}
			}
		}

		public function onScheduledUpdate() : void {
			if ($this->leafState !== DripleafState::STABLE) {
				if ($this->leafState === DripleafState::FULL_TILT) {
					$this->position->getWorld()->setBlock($this->position, $this->setLeafState(DripleafState::STABLE));
					if (\class_exists('pocketmine\world\sound\DripleafTiltUpSound')) {
						$this->position->getWorld()->addSound($this->position, new \pocketmine\world\sound\DripleafTiltUpSound());
					}
				} else {
					$this->setTiltAndScheduleTick(match($this->leafState) {
						DripleafState::UNSTABLE => DripleafState::PARTIAL_TILT,
						DripleafState::PARTIAL_TILT => DripleafState::FULL_TILT,
					});
					if (\class_exists('pocketmine\world\sound\DripleafTiltDownSound')) {
						$this->position->getWorld()->addSound($this->position, new \pocketmine\world\sound\DripleafTiltDownSound());
					}
				}
			}
		}

		protected function recalculateCollisionBoxes() : array {
			if ($this->leafState !== DripleafState::FULL_TILT) {
				return [
					AxisAlignedBB::one()
						->trim(Facing::DOWN, 11 / 16)
						->trim(Facing::UP, $this->getLeafTopOffset())
				];
			}
			return [];
		}
	}

	class BigDripleafStem extends BaseBigDripleaf {

		protected function isHead() : bool {
			return false;
		}

		protected function recalculateCollisionBoxes() : array {
			return [];
		}

		public function asItem() : Item {
			return VanillaBlocks::BIG_DRIPLEAF_HEAD()->asItem();
		}
	}

	class SmallDripleaf extends Transparent implements HorizontalFacing {
		use HorizontalFacingTrait;

		protected bool $top = false;

		public function describeBlockOnlyState(RuntimeDataDescriber $w) : void {
			$w->horizontalFacing($this->facing);
			$w->bool($this->top);
		}

		public function isTop() : bool {
			return $this->top;
		}

		/** @return $this */
		public function setTop(bool $top) : self {
			$this->top = $top;
			return $this;
		}

		private function canBeSupportedBy(Block $block) : bool {
			return $block->getTypeId() === BlockTypeIds::CLAY ||
				$block->hasTypeTag(BlockTypeTags::DIRT) ||
				$block->hasTypeTag(BlockTypeTags::MUD);
		}

		public function onNearbyBlockChange() : void {
			if (!$this->top && !$this->canBeSupportedBy($this->getSide(Facing::DOWN))) {
				$this->position->getWorld()->useBreakOn($this->position);
				return;
			}
			$face = $this->top ? Facing::DOWN : Facing::UP;
			if (!$this->getSide($face)->hasSameTypeId($this)) {
				$this->position->getWorld()->useBreakOn($this->position);
			}
		}

		public function place(BlockTransaction $tx, Item $item, Block $blockReplace, Block $blockClicked, int $face, Vector3 $clickVector, ?Player $player = null) : bool {
			$block = $blockReplace->getSide(Facing::UP);
			if ($block->getTypeId() !== BlockTypeIds::AIR || !$this->canBeSupportedBy($blockReplace->getSide(Facing::DOWN))) {
				return false;
			}
			if ($player !== null) {
				$this->facing = Facing::opposite($player->getHorizontalFacing());
			}

			$tx->addBlock($block->position, VanillaBlocks::SMALL_DRIPLEAF()
				->setFacing($this->facing)
				->setTop(true)
			);
			return parent::place($tx, $item, $blockReplace, $blockClicked, $face, $clickVector, $player);
		}

		public function onInteract(Item $item, int $face, Vector3 $clickVector, ?Player $player = null, array &$returnedItems = []) : bool {
			if ($item instanceof Fertilizer || \str_contains(\strtolower($item->getName()), 'bone meal') || \str_contains(\strtolower($item->getName()), 'fertilizer')) {
				$item->pop();
				return true;
			}
			return false;
		}
	}

	// ------------------------------------------------------------------------
	// Azalea Family
	// ------------------------------------------------------------------------

	class Azalea extends Flowable {
		use StaticSupportTrait;

		public function onInteract(Item $item, int $face, Vector3 $clickVector, ?Player $player = null, array &$returnedItems = []) : bool {
			if ($item instanceof Fertilizer || \str_contains(\strtolower($item->getName()), 'bone meal') || \str_contains(\strtolower($item->getName()), 'fertilizer')) {
				$item->pop();
				if ($player === null || !$player->hasFiniteResources() || mt_rand(1, 100) <= 45) {
					$this->grow($player);
				}
				return true;
			}

			return false;
		}

		private function grow(?Player $player) : void {
			if (!\class_exists('pocketmine\world\generator\object\TreeFactory')) {
				return; // Native generator tree growth fallback
			}
			$random = new Random(mt_rand());
			$tree = TreeFactory::get($random, TreeType::AZALEA);
			$transaction = $tree?->getBlockTransaction($this->position->getWorld(), $this->position->getFloorX(), $this->position->getFloorY(), $this->position->getFloorZ(), $random);
			if ($transaction === null) {
				return;
			}

			$ev = new StructureGrowEvent($this, $transaction, $player);
			$ev->call();
			if (!$ev->isCancelled()) {
				$transaction->apply();
			}
		}

		protected function recalculateCollisionBoxes() : array {
			return [
				AxisAlignedBB::one()
					->squash(Axis::X, 6 / 16)
					->squash(Axis::Z, 6 / 16)
					->trim(Facing::UP, 8 / 16),
				AxisAlignedBB::one()->trim(Facing::DOWN, 8 / 16)
			];
		}

		private function canBeSupportedAt(Block $block) : bool {
			$supportBlock = $block->getSide(Facing::DOWN);
			return $supportBlock->getTypeId() === BlockTypeIds::CLAY ||
				$supportBlock->hasTypeTag(BlockTypeTags::DIRT) ||
				$supportBlock->hasTypeTag(BlockTypeTags::MUD);
		}
	}

	class FloweringAzalea extends Azalea {
	}

	// ------------------------------------------------------------------------
	// Flowers & Grasses
	// ------------------------------------------------------------------------

	class Flower extends Flowable {
		use StaticSupportTrait;

		private function canBeSupportedAt(Block $block) : bool {
			$supportBlock = $block->getSide(Facing::DOWN);
			return $supportBlock->hasTypeTag(BlockTypeTags::DIRT) || $supportBlock->hasTypeTag(BlockTypeTags::MUD);
		}

		public function getFlameEncouragement() : int {
			return 60;
		}

		public function getFlammability() : int {
			return 100;
		}
	}

	class WitherRose extends Flowable {
		use StaticSupportTrait;

		private function canBeSupportedAt(Block $block) : bool {
			$supportBlock = $block->getSide(Facing::DOWN);
			return
				$supportBlock->hasTypeTag(BlockTypeTags::DIRT) ||
				$supportBlock->hasTypeTag(BlockTypeTags::MUD) ||
				match($supportBlock->getTypeId()) {
					BlockTypeIds::NETHERRACK,
					BlockTypeIds::SOUL_SAND,
					BlockTypeIds::SOUL_SOIL => true,
					default => false,
				};
		}

		public function hasEntityCollision() : bool { return true; }

		public function onEntityInside(Entity $entity) : bool {
			if ($entity instanceof Living && \class_exists(VanillaEffects::class)) {
				if (!$entity->getEffects()->has(VanillaEffects::WITHER())) {
					$entity->getEffects()->add(new EffectInstance(VanillaEffects::WITHER(), 40));
				}
			}
			return true;
		}

		public function getFlameEncouragement() : int { return 60; }

		public function getFlammability() : int { return 100; }
	}

	class TallGrass extends Flowable {
		use TallGrassTrait;
		use StaticSupportTrait;

		/** @phpstan-var \Closure() : DoublePlant|null */
		private ?\Closure $doublePlantVariant;

		/**
		 * @phpstan-param \Closure() : DoublePlant|null $doublePlantVariant
		 */
		public function __construct(BlockIdentifier $idInfo, string $name, BlockTypeInfo $typeInfo, ?\Closure $doublePlantVariant = null) {
			parent::__construct($idInfo, $name, $typeInfo);
			$this->doublePlantVariant = $doublePlantVariant;
		}

		private function canBeSupportedAt(Block $block) : bool {
			$supportBlock = $block->getSide(Facing::DOWN);
			return $supportBlock->hasTypeTag(BlockTypeTags::DIRT) || $supportBlock->hasTypeTag(BlockTypeTags::MUD);
		}

		public function onInteract(Item $item, int $face, Vector3 $clickVector, ?Player $player = null, array &$returnedItems = []) : bool {
			$world = $this->position->getWorld();
			$upPos = $this->position->getSide(Facing::UP);
			if (!$world->isInWorld($upPos->getFloorX(), $upPos->getFloorY(), $upPos->getFloorZ()) || $this->getSide(Facing::UP)->getTypeId() !== BlockTypeIds::AIR) {
				return false;
			}

			if (($item instanceof Fertilizer || \str_contains(\strtolower($item->getName()), 'bone meal') || \str_contains(\strtolower($item->getName()), 'fertilizer')) && ($doubleVariant = $this->getDoublePlantVariant()) !== null) {
				$bottom = (clone $doubleVariant)->setTop(false);
				$top = (clone $doubleVariant)->setTop(true);
				$world->setBlock($this->position, $bottom);
				$world->setBlock($this->position->getSide(Facing::UP), $top);
				$item->pop();

				return true;
			}

			return false;
		}

		private function getDoublePlantVariant() : ?DoublePlant {
			return $this->doublePlantVariant !== null ? ($this->doublePlantVariant)() : null;
		}
	}

	class DoublePlant extends Flowable {
		protected bool $top = false;

		protected function describeBlockOnlyState(RuntimeDataDescriber $w) : void {
			$w->bool($this->top);
		}

		public function isTop() : bool { return $this->top; }

		/** @return $this */
		public function setTop(bool $top) : self {
			$this->top = $top;
			return $this;
		}

		public function place(BlockTransaction $tx, Item $item, Block $blockReplace, Block $blockClicked, int $face, Vector3 $clickVector, ?Player $player = null) : bool {
			$down = $blockReplace->getSide(Facing::DOWN);
			if ($down->hasTypeTag(BlockTypeTags::DIRT) && $blockReplace->getSide(Facing::UP)->canBeReplaced()) {
				$top = clone $this;
				$top->top = true;
				$tx->addBlock($blockReplace->position, $this)->addBlock($blockReplace->position->getSide(Facing::UP), $top);
				return true;
			}

			return false;
		}

		public function isValidHalfPlant() : bool {
			$other = $this->getSide($this->top ? Facing::DOWN : Facing::UP);

			return (
				$other instanceof DoublePlant &&
				$other->hasSameTypeId($this) &&
				$other->top !== $this->top
			);
		}

		public function onNearbyBlockChange() : void {
			$down = $this->getSide(Facing::DOWN);
			if (!$this->isValidHalfPlant() || (!$this->top && !$down->hasTypeTag(BlockTypeTags::DIRT) && !$down->hasTypeTag(BlockTypeTags::MUD))) {
				$this->position->getWorld()->useBreakOn($this->position);
			}
		}

		public function getDrops(mixed $item = null) : array {
			return $this->top ? parent::getDrops($item) : [];
		}

		public function getAffectedBlocks() : array {
			if ($this->isValidHalfPlant()) {
				return [$this, $this->getSide($this->top ? Facing::DOWN : Facing::UP)];
			}

			return parent::getAffectedBlocks();
		}

		public function getFlameEncouragement() : int {
			return 60;
		}

		public function getFlammability() : int {
			return 100;
		}
	}

	class DoubleTallGrass extends DoublePlant {
		use TallGrassTrait {
			getDropsForIncompatibleTool as traitGetDropsForIncompatibleTool;
		}

		public function canBeReplaced() : bool {
			return true;
		}

		public function getDropsForIncompatibleTool(Item $item) : array {
			if ($this->top) {
				return $this->traitGetDropsForIncompatibleTool($item);
			}
			return [];
		}
	}

	final class DoublePitcherCrop extends DoublePlant implements Ageable {
		use AgeableTrait {
			describeBlockOnlyState as describeAge;
		}

		public const MAX_AGE = 1;

		public function describeBlockOnlyState(RuntimeDataDescriber $w) : void {
			parent::describeBlockOnlyState($w);
			$this->describeAge($w);
		}

		public function onInteract(Item $item, int $face, Vector3 $clickVector, ?Player $player = null, array &$returnedItems = []) : bool {
			if ($item instanceof Fertilizer || \str_contains(\strtolower($item->getName()), 'bone meal') || \str_contains(\strtolower($item->getName()), 'fertilizer')) {
				if ($this->age < self::MAX_AGE) {
					$world = $this->position->getWorld();
					$tx = new BlockTransaction($world);
					$other = $this->getSide($this->top ? Facing::DOWN : Facing::UP);
					$tx->addBlock($this->position, (clone $this)->setAge($this->age + 1));
					if ($other instanceof DoublePitcherCrop) {
						$tx->addBlock($other->position, (clone $other)->setAge($other->age + 1));
					}
					$ev = new StructureGrowEvent($this, $tx, $player);
					$ev->call();
					if (!$ev->isCancelled() && $tx->apply()) {
						$item->pop();
						return true;
					}
				}
			}
			return false;
		}

		public function getDropsForCompatibleTool(Item $item) : array {
			if ($this->top) {
				return [VanillaBlocks::PITCHER_PLANT()->asItem()];
			}
			return [];
		}
	}

	class DeadBush extends Flowable {
		use StaticSupportTrait;

		public function getDropsForIncompatibleTool(Item $item) : array {
			return [
				VanillaItems::STICK()->setCount(mt_rand(0, 2))
			];
		}

		public function isAffectedBySilkTouch() : bool {
			return true;
		}

		public function getFlameEncouragement() : int {
			return 60;
		}

		public function getFlammability() : int {
			return 100;
		}

		private function canBeSupportedAt(Block $block) : bool {
			$down = $block->getSide(Facing::DOWN);
			return $down->hasTypeTag(BlockTypeTags::SAND) ||
				$down->hasTypeTag(BlockTypeTags::DIRT) ||
				$down->getTypeId() === BlockTypeIds::HARDENED_CLAY;
		}
	}

	class WaterLily extends Flowable {
		use StaticSupportTrait {
			canBePlacedAt as supportedWhenPlacedAt;
		}

		protected function recalculateCollisionBoxes() : array {
			return [AxisAlignedBB::one()->contract(1 / 16, 0, 1 / 16)->trim(Facing::UP, 63 / 64)];
		}

		public function canBePlacedAt(Block $blockReplace, Vector3 $clickVector, int $face, bool $isClickedBlock) : bool {
			return !$blockReplace instanceof Water && $this->supportedWhenPlacedAt($blockReplace, $clickVector, $face, $isClickedBlock);
		}

		private function canBeSupportedAt(Block $block) : bool {
			return $block->getSide(Facing::DOWN) instanceof Water || $block->getSide(Facing::DOWN)->getTypeId() === BlockTypeIds::WATER;
		}
	}

	class PinkPetals extends Flowable implements HorizontalFacing {
		use HorizontalFacingTrait;
		use StaticSupportTrait {
			canBePlacedAt as supportedWhenPlacedAt;
		}

		public const MIN_COUNT = 1;
		public const MAX_COUNT = 4;

		protected int $count = self::MIN_COUNT;

		protected function describeBlockOnlyState(RuntimeDataDescriber $w) : void {
			$w->horizontalFacing($this->facing);
			if (\method_exists($w, 'boundedIntAuto')) {
				$w->boundedIntAuto(self::MIN_COUNT, self::MAX_COUNT, $this->count);
			}
		}

		public function getCount() : int {
			return $this->count;
		}

		/** @return $this */
		public function setCount(int $count) : self {
			if ($count < self::MIN_COUNT || $count > self::MAX_COUNT) {
				throw new \InvalidArgumentException("Count must be in range " . self::MIN_COUNT . " ... " . self::MAX_COUNT);
			}
			$this->count = $count;
			return $this;
		}

		private function canBeSupportedAt(Block $block) : bool {
			$supportBlock = $block->getSide(Facing::DOWN);
			return $supportBlock->hasTypeTag(BlockTypeTags::DIRT) || $supportBlock->hasTypeTag(BlockTypeTags::MUD);
		}

		public function canBePlacedAt(Block $blockReplace, Vector3 $clickVector, int $face, bool $isClickedBlock) : bool {
			return ($blockReplace instanceof PinkPetals && $blockReplace->count < self::MAX_COUNT) || $this->supportedWhenPlacedAt($blockReplace, $clickVector, $face, $isClickedBlock);
		}

		public function place(BlockTransaction $tx, Item $item, Block $blockReplace, Block $blockClicked, int $face, Vector3 $clickVector, ?Player $player = null) : bool {
			if ($blockReplace instanceof PinkPetals && $blockReplace->count < self::MAX_COUNT) {
				$this->count = $blockReplace->count + 1;
				$this->facing = $blockReplace->facing;
			} elseif ($player !== null) {
				$this->facing = Facing::opposite($player->getHorizontalFacing());
			}
			return parent::place($tx, $item, $blockReplace, $blockClicked, $face, $clickVector, $player);
		}

		public function onInteract(Item $item, int $face, Vector3 $clickVector, ?Player $player = null, array &$returnedItems = []) : bool {
			if ($item instanceof Fertilizer || \str_contains(\strtolower($item->getName()), 'bone meal') || \str_contains(\strtolower($item->getName()), 'fertilizer')) {
				$grew = false;
				if ($this->count < self::MAX_COUNT) {
					$grew = BlockEventHelper::grow($this, (clone $this)->setCount($this->count + 1), $player);
				} else {
					$this->position->getWorld()->dropItem($this->position->add(0, 0.5, 0), $this->asItem());
					$grew = true;
				}
				if ($grew) {
					$item->pop();
					return true;
				}
			}
			return false;
		}

		public function getDropsForCompatibleTool(Item $item) : array {
			return [$this->asItem()->setCount($this->count)];
		}
	}

	final class SporeBlossom extends Flowable {
		use StaticSupportTrait;

		private function canBeSupportedAt(Block $block) : bool {
			return $block->getAdjacentSupportType(Facing::UP) === SupportType::FULL;
		}
	}

	final class HangingRoots extends Flowable {
		use StaticSupportTrait;

		private function canBeSupportedAt(Block $block) : bool {
			return $block->getAdjacentSupportType(Facing::UP)->hasCenterSupport();
		}

		public function getDropsForIncompatibleTool(Item $item) : array {
			if (\class_exists('pocketmine\item\enchantment\VanillaEnchantments')) {
				if ($item->hasEnchantment(\pocketmine\item\enchantment\VanillaEnchantments::SILK_TOUCH())) {
					return $this->getDropsForCompatibleTool($item);
				}
			}
			return [];
		}
	}

	final class NetherRoots extends Flowable {
		use StaticSupportTrait;

		private function canBeSupportedAt(Block $block) : bool {
			$supportBlock = $block->getSide(Facing::DOWN);
			return
				$supportBlock->hasTypeTag(BlockTypeTags::DIRT) ||
				$supportBlock->hasTypeTag(BlockTypeTags::MUD) ||
				$supportBlock->hasTypeTag(BlockTypeTags::NYLIUM) ||
				$supportBlock->getTypeId() === BlockTypeIds::SOUL_SOIL;
		}
	}

	class NetherSprouts extends Flowable {
		use StaticSupportTrait;

		private function canBeSupportedAt(Block $block) : bool {
			$supportBlock = $block->getSide(Facing::DOWN);
			return
				$supportBlock->hasTypeTag(BlockTypeTags::DIRT) ||
				$supportBlock->hasTypeTag(BlockTypeTags::MUD) ||
				$supportBlock->hasTypeTag(BlockTypeTags::NYLIUM) ||
				$supportBlock->getTypeId() === BlockTypeIds::SOUL_SOIL;
		}
	}

	class NetherFungus extends Flowable {
		use StaticSupportTrait;

		public function __construct(
			BlockIdentifier $idInfo,
			string $name,
			BlockTypeInfo $typeInfo,
			private readonly int $nyliumTypeId = BlockTypeIds::CRIMSON_NYLIUM,
			private readonly ?TreeType $treeType = null
		) {
			parent::__construct($idInfo, $name, $typeInfo);
		}

		private function canBeSupportedAt(Block $block) : bool {
			$down = $block->getSide(Facing::DOWN);
			return $down->getTypeId() === $this->nyliumTypeId ||
				$down->hasTypeTag(BlockTypeTags::DIRT) ||
				$down->hasTypeTag(BlockTypeTags::MUD) ||
				$down->getTypeId() === BlockTypeIds::SOUL_SOIL;
		}

		public function onInteract(Item $item, int $face, Vector3 $clickVector, ?Player $player = null, array &$returnedItems = []) : bool {
			if ($item instanceof Fertilizer || \str_contains(\strtolower($item->getName()), 'bone meal') || \str_contains(\strtolower($item->getName()), 'fertilizer')) {
				$item->pop();
				if ($this->getSide(Facing::DOWN)->getTypeId() === $this->nyliumTypeId && ($player === null || !$player->hasFiniteResources() || mt_rand(1, 100) <= 40)) {
					$this->grow($player);
				}
				return true;
			}
			return false;
		}

		private function grow(?Player $player) : void {
			if (!\class_exists('pocketmine\world\generator\object\TreeFactory') || $this->treeType === null) {
				return;
			}
			$random = new Random(mt_rand());
			$tree = TreeFactory::get($random, $this->treeType);
			$transaction = $tree?->getBlockTransaction($this->position->getWorld(), $this->position->getFloorX(), $this->position->getFloorY(), $this->position->getFloorZ(), $random);
			if ($transaction === null) {
				return;
			}

			$ev = new StructureGrowEvent($this, $transaction, $player);
			$ev->call();
			if (!$ev->isCancelled()) {
				$transaction->apply();
			}
		}
	}

	// ------------------------------------------------------------------------
	// Mushrooms & Mushroom Blocks
	// ------------------------------------------------------------------------

	abstract class Mushroom extends Flowable {

		public function ticksRandomly() : bool {
			return true;
		}

		public function onNearbyBlockChange() : void {
			if ($this->getSide(Facing::DOWN)->isTransparent()) {
				$this->position->getWorld()->useBreakOn($this->position);
			}
		}

		public function place(BlockTransaction $tx, Item $item, Block $blockReplace, Block $blockClicked, int $face, Vector3 $clickVector, ?Player $player = null) : bool {
			$down = $this->getSide(Facing::DOWN);
			$position = $this->position;
			$lightLevel = $position->getWorld()->getFullLightAt($position->x, $position->y, $position->z);
			$downId = $down->getTypeId();
			if (($lightLevel <= 12 && !$down->isTransparent()) || $downId === BlockTypeIds::MYCELIUM || $downId === BlockTypeIds::PODZOL || $down->hasTypeTag(BlockTypeTags::NYLIUM)) {
				return parent::place($tx, $item, $blockReplace, $blockClicked, $face, $clickVector, $player);
			}

			return false;
		}
	}

	class RedMushroom extends Mushroom {
	}

	class BrownMushroom extends Mushroom {

		public function getLightLevel() : int {
			return 1;
		}
	}

	final class MushroomStem extends Opaque {

		public function getDropsForCompatibleTool(Item $item) : array { return []; }

		public function isAffectedBySilkTouch() : bool { return true; }
	}

	class RedMushroomBlock extends Opaque {
		protected MushroomBlockType $mushroomBlockType = MushroomBlockType::ALL_CAP;

		public function describeBlockItemState(RuntimeDataDescriber $w) : void {
			if (\method_exists($w, 'enum')) {
				$w->enum($this->mushroomBlockType);
			}
		}

		public function getMushroomBlockType() : MushroomBlockType { return $this->mushroomBlockType; }

		/** @return $this */
		public function setMushroomBlockType(MushroomBlockType $mushroomBlockType) : self {
			$this->mushroomBlockType = $mushroomBlockType;
			return $this;
		}

		public function getDropsForCompatibleTool(Item $item) : array {
			return [
				VanillaBlocks::RED_MUSHROOM()->asItem()->setCount(mt_rand(0, 2))
			];
		}

		public function isAffectedBySilkTouch() : bool {
			return true;
		}

		public function getSilkTouchDrops(Item $item) : array {
			return [(clone $this)->setMushroomBlockType(MushroomBlockType::ALL_CAP)->asItem()];
		}

		public function getPickedItem(bool $addUserData = false) : Item {
			return (clone $this)->setMushroomBlockType(MushroomBlockType::ALL_CAP)->asItem();
		}
	}

	class BrownMushroomBlock extends RedMushroomBlock {

		public function getDropsForCompatibleTool(Item $item) : array {
			return [
				VanillaBlocks::BROWN_MUSHROOM()->asItem()->setCount(mt_rand(0, 2))
			];
		}
	}

	// ------------------------------------------------------------------------
	// SweetBerryBush, NetherVines, CaveVines, Kelp, Seagrass, MangroveRoots
	// ------------------------------------------------------------------------

	class SweetBerryBush extends Flowable implements Ageable {
		use AgeableTrait;
		use StaticSupportTrait;

		public const STAGE_SAPLING = 0;
		public const STAGE_BUSH_NO_BERRIES = 1;
		public const STAGE_BUSH_SOME_BERRIES = 2;
		public const STAGE_MATURE = 3;
		public const MAX_AGE = self::STAGE_MATURE;

		public function getBerryDropAmount() : int {
			if ($this->age === self::STAGE_MATURE) {
				return mt_rand(2, 3);
			} elseif ($this->age >= self::STAGE_BUSH_SOME_BERRIES) {
				return mt_rand(1, 2);
			}
			return 0;
		}

		protected function canBeSupportedBy(Block $block) : bool {
			return $block->getTypeId() !== BlockTypeIds::FARMLAND &&
				($block->hasTypeTag(BlockTypeTags::DIRT) || $block->hasTypeTag(BlockTypeTags::MUD));
		}

		private function canBeSupportedAt(Block $block) : bool {
			$supportBlock = $block->getSide(Facing::DOWN);
			return $this->canBeSupportedBy($supportBlock);
		}

		public function onInteract(Item $item, int $face, Vector3 $clickVector, ?Player $player = null, array &$returnedItems = []) : bool {
			$world = $this->position->getWorld();
			if ($this->age < self::STAGE_MATURE && ($item instanceof Fertilizer || \str_contains(\strtolower($item->getName()), 'bone meal') || \str_contains(\strtolower($item->getName()), 'fertilizer'))) {
				$block = clone $this;
				$block->age++;
				if (BlockEventHelper::grow($this, $block, $player)) {
					$item->pop();
				}
			} elseif (($dropAmount = $this->getBerryDropAmount()) > 0) {
				$world->setBlock($this->position, $this->setAge(self::STAGE_BUSH_NO_BERRIES));
				$world->dropItem($this->position, $this->asItem()->setCount($dropAmount));
				if (\class_exists('pocketmine\world\sound\SweetBerriesPickSound')) {
					$world->addSound($this->position, new \pocketmine\world\sound\SweetBerriesPickSound());
				}
			}

			return true;
		}

		public function asItem() : Item {
			return VanillaItems::SWEET_BERRIES();
		}

		public function getDropsForCompatibleTool(Item $item) : array {
			$count = match($this->age) {
				self::STAGE_MATURE => FortuneDropHelper::discrete($item, 2, 3),
				self::STAGE_BUSH_SOME_BERRIES => FortuneDropHelper::discrete($item, 1, 2),
				default => 0,
			};
			return [
				$this->asItem()->setCount($count)
			];
		}

		public function ticksRandomly() : bool {
			return $this->age < self::STAGE_MATURE;
		}

		public function onRandomTick() : void {
			if ($this->age < self::STAGE_MATURE && mt_rand(0, 2) === 1) {
				$block = clone $this;
				++$block->age;
				BlockEventHelper::grow($this, $block, null);
			}
		}

		public function hasEntityCollision() : bool {
			return true;
		}

		public function onEntityInside(Entity $entity) : bool {
			if ($this->age >= self::STAGE_BUSH_NO_BERRIES && $entity instanceof Living) {
				$entity->resetFallDistance();
				$entity->attack(new EntityDamageByBlockEvent($this, $entity, EntityDamageEvent::CAUSE_CONTACT, 1));
			}
			return true;
		}
	}

	class NetherVines extends Flowable implements Ageable {
		use AgeableTrait;
		use StaticSupportTrait;

		public const MAX_AGE = 25;

		private int $growthFace;

		public function __construct(BlockIdentifier $idInfo, string $name, BlockTypeInfo $typeInfo, int $growthFace = Facing::DOWN) {
			$this->growthFace = $growthFace;
			parent::__construct($idInfo, $name, $typeInfo);
		}

		public function isAffectedBySilkTouch() : bool {
			return true;
		}

		public function canClimb() : bool {
			return true;
		}

		private function canBeSupportedAt(Block $block) : bool {
			$supportBlock = $block->getSide(Facing::opposite($this->growthFace));
			return $supportBlock->getSupportType($this->growthFace)->hasCenterSupport() || $supportBlock->hasSameTypeId($this);
		}

		private function seekToTip() : NetherVines {
			$top = $this;
			while (($next = $top->getSide($this->growthFace)) instanceof NetherVines && $next->hasSameTypeId($this)) {
				$top = $next;
			}
			return $top;
		}

		public function place(BlockTransaction $tx, Item $item, Block $blockReplace, Block $blockClicked, int $face, Vector3 $clickVector, ?Player $player = null) : bool {
			$this->age = mt_rand(0, self::MAX_AGE - 1);
			return parent::place($tx, $item, $blockReplace, $blockClicked, $face, $clickVector, $player);
		}

		public function onInteract(Item $item, int $face, Vector3 $clickVector, ?Player $player = null, array &$returnedItems = []) : bool {
			if ($item instanceof Fertilizer || \str_contains(\strtolower($item->getName()), 'bone meal') || \str_contains(\strtolower($item->getName()), 'fertilizer')) {
				if ($this->grow($player, mt_rand(1, 5))) {
					$item->pop();
				}
				return true;
			}
			return false;
		}

		public function ticksRandomly() : bool {
			return $this->age < self::MAX_AGE;
		}

		public function onRandomTick() : void {
			if ($this->age < self::MAX_AGE && mt_rand(1, 10) === 1) {
				$this->grow(null, 1);
			}
		}

		private function grow(?Player $player, int $length) : bool {
			$tip = $this->seekToTip();
			if ($tip->age >= self::MAX_AGE) {
				return false;
			}

			$world = $this->position->getWorld();
			$tx = new BlockTransaction($world);

			$current = $tip;
			for ($i = 0; $i < $length; $i++) {
				$nextPos = $current->position->getSide($this->growthFace);
				if (!$world->isInWorld($nextPos->getFloorX(), $nextPos->getFloorY(), $nextPos->getFloorZ())) {
					break;
				}
				$nextBlock = $world->getBlock($nextPos);
				if (!$nextBlock->canBeReplaced()) {
					break;
				}

				$newAge = min(self::MAX_AGE, $current->age + mt_rand(1, 2));
				$next = (clone $this)->setAge($newAge);
				$tx->addBlock($nextPos, $next);
				$current = $next;
				if ($newAge >= self::MAX_AGE) {
					break;
				}
			}

			$ev = new StructureGrowEvent($tip, $tx, $player);
			$ev->call();
			if (!$ev->isCancelled()) {
				return $tx->apply();
			}
			return false;
		}
	}

	class WeepingVines extends NetherVines {
		public function __construct(BlockIdentifier $idInfo, string $name, BlockTypeInfo $typeInfo, int $growthFace = Facing::DOWN) {
			parent::__construct($idInfo, $name, $typeInfo, Facing::DOWN);
		}
	}

	class TwistingVines extends NetherVines {
		public function __construct(BlockIdentifier $idInfo, string $name, BlockTypeInfo $typeInfo, int $growthFace = Facing::UP) {
			parent::__construct($idInfo, $name, $typeInfo, Facing::UP);
		}
	}

	class CaveVines extends Flowable implements Ageable {
		use AgeableTrait;
		use StaticSupportTrait;

		public const MAX_AGE = 25;

		protected bool $berries = false;
		protected bool $head = false;

		protected function describeBlockOnlyState(RuntimeDataDescriber $w) : void {
			if (\method_exists($w, 'boundedIntAuto')) {
				$w->boundedIntAuto(0, self::MAX_AGE, $this->age);
			}
			$w->bool($this->berries);
			$w->bool($this->head);
		}

		public function hasBerries() : bool { return $this->berries; }

		/** @return $this */
		public function setBerries(bool $berries) : self {
			$this->berries = $berries;
			return $this;
		}

		public function isHead() : bool { return $this->head; }

		/** @return $this */
		public function setHead(bool $head) : self {
			$this->head = $head;
			return $this;
		}

		public function canClimb() : bool {
			return true;
		}

		public function getLightLevel() : int {
			return $this->berries ? 14 : 0;
		}

		private function canBeSupportedAt(Block $block) : bool {
			$supportBlock = $block->getSide(Facing::UP);
			return $supportBlock->getSupportType(Facing::DOWN) === SupportType::FULL || $supportBlock->hasSameTypeId($this);
		}

		public function place(BlockTransaction $tx, Item $item, Block $blockReplace, Block $blockClicked, int $face, Vector3 $clickVector, ?Player $player = null) : bool {
			$this->age = mt_rand(0, self::MAX_AGE);
			return parent::place($tx, $item, $blockReplace, $blockClicked, $face, $clickVector, $player);
		}

		public function onInteract(Item $item, int $face, Vector3 $clickVector, ?Player $player = null, array &$returnedItems = []) : bool {
			if ($this->berries) {
				$this->position->getWorld()->dropItem($this->position, $this->asItem());
				if (\class_exists('pocketmine\world\sound\GlowBerriesPickSound')) {
					$this->position->getWorld()->addSound($this->position, new \pocketmine\world\sound\GlowBerriesPickSound());
				}

				$this->position->getWorld()->setBlock($this->position, $this->setBerries(false));
				return true;
			}
			if ($item instanceof Fertilizer || \str_contains(\strtolower($item->getName()), 'bone meal') || \str_contains(\strtolower($item->getName()), 'fertilizer')) {
				if (!$this->berries) {
					$this->position->getWorld()->setBlock($this->position, $this->setBerries(true));
					$item->pop();
					return true;
				}
			}
			return false;
		}

		public function ticksRandomly() : bool {
			return $this->age < self::MAX_AGE;
		}

		public function onRandomTick() : void {
			if ($this->head && $this->age < self::MAX_AGE && mt_rand(1, 10) === 1) {
				$down = $this->getSide(Facing::DOWN);
				if ($down->canBeReplaced()) {
					$world = $this->position->getWorld();
					$hasBerries = mt_rand(1, 9) === 1;
					$newHead = (clone $this)->setAge($this->age + 1)->setBerries($hasBerries)->setHead(true);
					$this->setHead(false);
					$world->setBlock($this->position, $this);
					$world->setBlock($down->position, $newHead);
				}
			}
		}

		public function asItem() : Item {
			return VanillaItems::GLOW_BERRIES();
		}
	}

	final class MangroveRoots extends Transparent {

		public function getFlammability() : int { return 5; }

		public function getFlameEncouragement() : int { return 5; }
	}

	class Kelp extends Flowable implements Ageable {
		use AgeableTrait;
		use StaticSupportTrait;

		public const MAX_AGE = 25;

		public function getDropsForCompatibleTool(Item $item) : array {
			return [$this->asItem()];
		}

		public function isAffectedBySilkTouch() : bool {
			return true;
		}

		private function canBeSupportedAt(Block $block) : bool {
			$support = $block->getSide(Facing::DOWN);
			return $support->hasSameTypeId($this) || $support->isSolid();
		}
	}

	class Seagrass extends Flowable {
		use StaticSupportTrait;

		public function getDropsForCompatibleTool(Item $item) : array {
			if (($item->getBlockToolType() & BlockToolType::SHEARS) !== 0) {
				return [$this->asItem()];
			}
			return [];
		}

		public function isAffectedBySilkTouch() : bool {
			return true;
		}

		private function canBeSupportedAt(Block $block) : bool {
			return $block->getSide(Facing::DOWN)->isSolid();
		}
	}
}
