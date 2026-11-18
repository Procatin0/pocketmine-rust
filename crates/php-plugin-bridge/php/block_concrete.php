<?php

declare(strict_types=1);

namespace pocketmine\entity {
    if (!interface_exists(Consumable::class, false)) {
        interface Consumable {
            public function getAdditionalEffects(): array;
            public function onConsume(Living $consumer): void;
        }
    }
    if (!interface_exists(FoodSource::class, false)) {
        interface FoodSource extends Consumable {
            public function getFoodRestore(): int;
            public function getSaturationRestore(): float;
            public function requiresHunger(): bool;
        }
    }
}
namespace pocketmine\block {
use pocketmine\block\utils\AmethystTrait;
use pocketmine\block\utils\AnyFacing;
use pocketmine\block\utils\AnyFacingTrait;
use pocketmine\block\utils\FortuneDropHelper;
use pocketmine\block\utils\SupportType;
use pocketmine\data\runtime\RuntimeDataDescriber;
use pocketmine\item\Item;
use pocketmine\item\VanillaItems;
use pocketmine\math\Axis;
use pocketmine\math\AxisAlignedBB;
use pocketmine\math\Facing;
use pocketmine\math\Vector3;
use pocketmine\player\Player;
use pocketmine\utils\AssumptionFailedError;
use pocketmine\world\BlockTransaction;

final class AmethystCluster extends Transparent implements AnyFacing{
	use AmethystTrait;
	use AnyFacingTrait;

	public const STAGE_SMALL_BUD = 0;
	public const STAGE_MEDIUM_BUD = 1;
	public const STAGE_LARGE_BUD = 2;
	public const STAGE_CLUSTER = 3;

	private int $stage = self::STAGE_CLUSTER;

	public function describeBlockItemState(RuntimeDataDescriber $w) : void{
		$w->boundedIntAuto(self::STAGE_SMALL_BUD, self::STAGE_CLUSTER, $this->stage);
	}

	public function getStage() : int{ return $this->stage; }

	public function setStage(int $stage) : self{
		if($stage < self::STAGE_SMALL_BUD || $stage > self::STAGE_CLUSTER){
			throw new \InvalidArgumentException("Size must be in range " . self::STAGE_SMALL_BUD . " ... " . self::STAGE_CLUSTER);
		}
		$this->stage = $stage;
		return $this;
	}

	public function getLightLevel() : int{
		return match($this->stage){
			self::STAGE_SMALL_BUD => 1,
			self::STAGE_MEDIUM_BUD => 2,
			self::STAGE_LARGE_BUD => 4,
			self::STAGE_CLUSTER => 5,
			default => throw new AssumptionFailedError("Invalid stage $this->stage"),
		};
	}

	protected function recalculateCollisionBoxes() : array{
		$myAxis = Facing::axis($this->facing);

		$box = AxisAlignedBB::one();
		foreach([Axis::Y, Axis::Z, Axis::X] as $axis){
			if($axis === $myAxis){
				continue;
			}
			$box->squash($axis, $this->stage === self::STAGE_SMALL_BUD ? 4 / 16 : 3 / 16);
		}
		$box->trim($this->facing, 1 - ($this->stage === self::STAGE_CLUSTER ? 7 / 16 : ($this->stage + 3) / 16));

		return [$box];
	}

	private function canBeSupportedAt(Block $block, int $facing) : bool{
		return $block->getAdjacentSupportType($facing) === SupportType::FULL;
	}

	public function getSupportType(int $facing) : SupportType{
		return SupportType::NONE;
	}

	public function place(BlockTransaction $tx, Item $item, Block $blockReplace, Block $blockClicked, int $face, Vector3 $clickVector, ?Player $player = null) : bool{
		if(!$this->canBeSupportedAt($blockReplace, Facing::opposite($face))){
			return false;
		}

		$this->facing = $face;
		return parent::place($tx, $item, $blockReplace, $blockClicked, $face, $clickVector, $player);
	}

	public function onNearbyBlockChange() : void{
		if(!$this->canBeSupportedAt($this, Facing::opposite($this->facing))){
			$this->position->getWorld()->useBreakOn($this->position);
		}
	}

	public function isAffectedBySilkTouch() : bool{
		return true;
	}

	public function getDropsForCompatibleTool(Item $item) : array{
		if($this->stage === self::STAGE_CLUSTER){
			return [VanillaItems::AMETHYST_SHARD()->setCount(FortuneDropHelper::weighted($item, min: 4, maxBase: 4))];
		}

		return [];
	}

	public function getDropsForIncompatibleTool(Item $item) : array{
		if($this->stage === self::STAGE_CLUSTER){
			return [VanillaItems::AMETHYST_SHARD()->setCount(FortuneDropHelper::weighted($item, min: 2, maxBase: 2))];
		}

		return [];
	}
}
}

namespace pocketmine\block {
use pocketmine\block\tile\Banner as TileBanner;
use pocketmine\block\utils\BannerPatternLayer;
use pocketmine\block\utils\Colored;
use pocketmine\block\utils\ColoredTrait;
use pocketmine\block\utils\SupportType;
use pocketmine\item\Banner as ItemBanner;
use pocketmine\item\Item;
use pocketmine\item\VanillaItems;
use pocketmine\math\Vector3;
use pocketmine\player\Player;
use pocketmine\world\BlockTransaction;
use function assert;
use function count;

abstract class BaseBanner extends Transparent implements Colored{
	use ColoredTrait;

	protected array $patterns = [];

	public function readStateFromWorld() : Block{
		parent::readStateFromWorld();
		$tile = $this->position->getWorld()->getTile($this->position);
		if($tile instanceof TileBanner){
			if($tile->getType() === TileBanner::TYPE_OMINOUS){
				//illager banner is implemented as a separate block, as it doesn't support base color or custom patterns
				return $this->getOminousVersion();
			}
			$this->color = $tile->getBaseColor();
			$this->setPatterns($tile->getPatterns());
		}

		return $this;
	}

	protected function getOminousVersion() : Block{
		return VanillaBlocks::AIR();
	}

	public function writeStateToWorld() : void{
		parent::writeStateToWorld();
		$tile = $this->position->getWorld()->getTile($this->position);
		assert($tile instanceof TileBanner);
		$tile->setBaseColor($this->color);
		$tile->setPatterns($this->patterns);
	}

	public function isSolid() : bool{
		return false;
	}

	public function getMaxStackSize() : int{
		return 16;
	}

	public function getPatterns() : array{
		return $this->patterns;
	}

	public function setPatterns(array $patterns) : self{
		foreach($patterns as $pattern){
			if(!$pattern instanceof BannerPatternLayer){
				throw new \TypeError("Array must only contain " . BannerPatternLayer::class . " objects");
			}
		}
		$this->patterns = $patterns;
		return $this;
	}

	protected function recalculateCollisionBoxes() : array{
		return [];
	}

	public function getSupportType(int $facing) : SupportType{
		return SupportType::NONE;
	}

	private function canBeSupportedBy(Block $block) : bool{
		return $block->isSolid();
	}

	public function place(BlockTransaction $tx, Item $item, Block $blockReplace, Block $blockClicked, int $face, Vector3 $clickVector, ?Player $player = null) : bool{
		if(!$this->canBeSupportedBy($blockReplace->getSide($this->getSupportingFace()))){
			return false;
		}
		if($item instanceof ItemBanner){
			$this->color = $item->getColor();
			$this->setPatterns($item->getPatterns());
		}

		return parent::place($tx, $item, $blockReplace, $blockClicked, $face, $clickVector, $player);
	}

	abstract protected function getSupportingFace() : int;

	public function onNearbyBlockChange() : void{
		if(!$this->canBeSupportedBy($this->getSide($this->getSupportingFace()))){
			$this->position->getWorld()->useBreakOn($this->position);
		}
	}

	public function getDropsForCompatibleTool(Item $item) : array{
		$drop = $this->asItem();
		if($drop instanceof ItemBanner && count($this->patterns) > 0){
			$drop->setPatterns($this->patterns);
		}

		return [$drop];
	}

	public function getPickedItem(bool $addUserData = false) : Item{
		$result = $this->asItem();
		if($addUserData && $result instanceof ItemBanner && count($this->patterns) > 0){
			$result->setPatterns($this->patterns);
		}
		return $result;
	}

	public function asItem() : Item{
		return VanillaItems::BANNER()->setColor($this->color);
	}
}
}

namespace pocketmine\block {
use pocketmine\block\utils\StaticSupportTrait;
use pocketmine\block\utils\SupportType;
use pocketmine\entity\effect\EffectInstance;
use pocketmine\entity\FoodSource;
use pocketmine\entity\Living;
use pocketmine\item\Item;
use pocketmine\math\Facing;
use pocketmine\math\Vector3;
use pocketmine\player\Player;

abstract class BaseCake extends Transparent implements FoodSource{
	use StaticSupportTrait;

	public function getSupportType(int $facing) : SupportType{
		return SupportType::NONE;
	}

	private function canBeSupportedAt(Block $block) : bool{
		return $block->getSide(Facing::DOWN)->getTypeId() !== BlockTypeIds::AIR;
	}

	public function onInteract(Item $item, int $face, Vector3 $clickVector, ?Player $player = null, array &$returnedItems = []) : bool{
		if($player !== null){
			return $player->consumeObject($this);
		}

		return false;
	}

	public function getFoodRestore() : int{
		return 2;
	}

	public function getSaturationRestore() : float{
		return 0.4;
	}

	public function requiresHunger() : bool{
		return true;
	}

	public function getAdditionalEffects() : array{
		return [];
	}

	abstract public function getResidue() : Block;

	public function onConsume(Living $consumer) : void{
		$this->position->getWorld()->setBlock($this->position, $this->getResidue());
	}
}
}

namespace pocketmine\block {
use pocketmine\block\utils\BlockEventHelper;
use pocketmine\block\utils\CoralMaterial;
use pocketmine\block\utils\CoralTypeTrait;
use pocketmine\block\utils\SupportType;
use pocketmine\item\Item;
use function mt_rand;

abstract class BaseCoral extends Transparent implements CoralMaterial{
	use CoralTypeTrait;

	public function onNearbyBlockChange() : void{
		if(!$this->dead){
			$this->position->getWorld()->scheduleDelayedBlockUpdate($this->position, mt_rand(40, 200));
		}
	}

	public function onScheduledUpdate() : void{
		if(!$this->dead && !$this->isCoveredWithWater()){
			BlockEventHelper::die($this, (clone $this)->setDead(true));
		}
	}

	public function getDropsForCompatibleTool(Item $item) : array{
		return [];
	}

	public function isAffectedBySilkTouch() : bool{
		return true;
	}

	public function isSolid() : bool{ return false; }

	protected function isCoveredWithWater() : bool{
		$world = $this->position->getWorld();

		$hasWater = false;
		foreach($this->position->sides() as $vector3){
			if($world->getBlock($vector3) instanceof Water){
				$hasWater = true;
				break;
			}
		}

		//TODO: check water inside the block itself (not supported on the API yet)
		return $hasWater;
	}

	protected function recalculateCollisionBoxes() : array{ return []; }

	public function getSupportType(int $facing) : SupportType{
		return SupportType::NONE;
	}
}
}

namespace pocketmine\block {
use pocketmine\block\tile\Banner as TileBanner;
use pocketmine\block\utils\DyeColor;
use pocketmine\block\utils\SupportType;
use pocketmine\item\Item;
use pocketmine\item\VanillaItems;
use pocketmine\math\Vector3;
use pocketmine\player\Player;
use pocketmine\world\BlockTransaction;
use function assert;

abstract class BaseOminousBanner extends Transparent{

	public function writeStateToWorld() : void{
		parent::writeStateToWorld();
		$tile = $this->position->getWorld()->getTile($this->position);
		assert($tile instanceof TileBanner);
		$tile->setBaseColor(DyeColor::WHITE);
		$tile->setPatterns([]);
		$tile->setType(TileBanner::TYPE_OMINOUS);
	}

	public function isSolid() : bool{
		return false;
	}

	public function getMaxStackSize() : int{
		return 16;
	}

	public function getFuelTime() : int{
		return 300;
	}

	protected function recalculateCollisionBoxes() : array{
		return [];
	}

	public function getSupportType(int $facing) : SupportType{
		return SupportType::NONE;
	}

	private function canBeSupportedBy(Block $block) : bool{
		return $block->isSolid();
	}

	public function place(BlockTransaction $tx, Item $item, Block $blockReplace, Block $blockClicked, int $face, Vector3 $clickVector, ?Player $player = null) : bool{
		if(!$this->canBeSupportedBy($blockReplace->getSide($this->getSupportingFace()))){
			return false;
		}

		return parent::place($tx, $item, $blockReplace, $blockClicked, $face, $clickVector, $player);
	}

	abstract protected function getSupportingFace() : int;

	public function onNearbyBlockChange() : void{
		if(!$this->canBeSupportedBy($this->getSide($this->getSupportingFace()))){
			$this->position->getWorld()->useBreakOn($this->position);
		}
	}

	public function asItem() : Item{
		return VanillaItems::OMINOUS_BANNER();
	}
}
}

namespace pocketmine\block {
use pocketmine\block\tile\Sign as TileSign;
use pocketmine\block\utils\DyeColor;
use pocketmine\block\utils\SignText;
use pocketmine\block\utils\SupportType;
use pocketmine\block\utils\WoodMaterial;
use pocketmine\block\utils\WoodType;
use pocketmine\block\utils\WoodTypeTrait;
use pocketmine\color\Color;
use pocketmine\event\block\SignChangeEvent;
use pocketmine\item\Dye;
use pocketmine\item\Item;
use pocketmine\item\ItemTypeIds;
use pocketmine\math\Vector3;
use pocketmine\player\Player;
use pocketmine\utils\TextFormat;
use pocketmine\world\BlockTransaction;
use pocketmine\world\sound\DyeUseSound;
use pocketmine\world\sound\InkSacUseSound;
use function abs;
use function array_map;
use function assert;
use function atan2;
use function fmod;
use function rad2deg;
use function strlen;

abstract class BaseSign extends Transparent implements WoodMaterial{
	use WoodTypeTrait;

	protected SignText $text; //TODO: rename this (BC break)
	protected SignText $backText;
	private bool $waxed = false;

	protected ?int $editorEntityRuntimeId = null;

	private \Closure $asItemCallback;

	public function __construct(BlockIdentifier $idInfo, string $name, BlockTypeInfo $typeInfo, WoodType $woodType, \Closure $asItemCallback){
		$this->woodType = $woodType;
		parent::__construct($idInfo, $name, $typeInfo);
		$this->text = new SignText();
		$this->backText = new SignText();
		$this->asItemCallback = $asItemCallback;
	}

	public function readStateFromWorld() : Block{
		parent::readStateFromWorld();
		$tile = $this->position->getWorld()->getTile($this->position);
		if($tile instanceof TileSign){
			$this->text = $tile->getText();
			$this->backText = $tile->getBackText();
			$this->waxed = $tile->isWaxed();
			$this->editorEntityRuntimeId = $tile->getEditorEntityRuntimeId();
		}

		return $this;
	}

	public function writeStateToWorld() : void{
		parent::writeStateToWorld();
		$tile = $this->position->getWorld()->getTile($this->position);
		assert($tile instanceof TileSign);
		$tile->setText($this->text);
		$tile->setBackText($this->backText);
		$tile->setWaxed($this->waxed);
		$tile->setEditorEntityRuntimeId($this->editorEntityRuntimeId);
	}

	public function isSolid() : bool{
		return false;
	}

	public function getMaxStackSize() : int{
		return 16;
	}

	protected function recalculateCollisionBoxes() : array{
		return [];
	}

	public function getSupportType(int $facing) : SupportType{
		return SupportType::NONE;
	}

	abstract protected function getSupportingFace() : int;

	public function onNearbyBlockChange() : void{
		if($this->getSide($this->getSupportingFace())->getTypeId() === BlockTypeIds::AIR){
			$this->position->getWorld()->useBreakOn($this->position);
		}
	}

	public function place(BlockTransaction $tx, Item $item, Block $blockReplace, Block $blockClicked, int $face, Vector3 $clickVector, ?Player $player = null) : bool{
		if($player !== null){
			$this->editorEntityRuntimeId = $player->getId();
		}
		return parent::place($tx, $item, $blockReplace, $blockClicked, $face, $clickVector, $player);
	}

	public function onPostPlace() : void{
		$player = $this->editorEntityRuntimeId !== null ?
			$this->position->getWorld()->getEntity($this->editorEntityRuntimeId) :
			null;
		//TODO: HACK! We really shouldn't be keeping disconnected players (and generally flagged-for-despawn entities)
		//in the world's entity table, but changing that is too risky for a hotfix. This workaround will do for now.
		if($player instanceof Player && $player->isConnected()){
			$player->openSignEditor($this->position);
		}
	}

	private function doSignChange(SignText $newText, Player $player, Item $item, bool $frontFace) : bool{
		$ev = new SignChangeEvent($this, $player, $newText, $frontFace);
		$ev->call();
		if(!$ev->isCancelled()){
			$this->setFaceText($frontFace, $ev->getNewText());
			$this->position->getWorld()->setBlock($this->position, $this);
			$item->pop();
			return true;
		}

		return false;
	}

	private function changeSignGlowingState(bool $glowing, Player $player, Item $item, bool $frontFace) : bool{
		$text = $this->getFaceText($frontFace);
		if($text->isGlowing() !== $glowing && $this->doSignChange(new SignText($text->getLines(), $text->getBaseColor(), $glowing), $player, $item, $frontFace)){
			$this->position->getWorld()->addSound($this->position, new InkSacUseSound());
			return true;
		}
		return false;
	}

	private function wax(Player $player, Item $item) : bool{
		if($this->waxed){
			return false;
		}

		$this->waxed = true;
		$this->position->getWorld()->setBlock($this->position, $this);
		$item->pop();

		return true;
	}

	public function onInteract(Item $item, int $face, Vector3 $clickVector, ?Player $player = null, array &$returnedItems = []) : bool{
		if($player === null){
			return false;
		}
		if($this->waxed){
			return true;
		}

		$frontFace = $this->interactsFront($this->getHitboxCenter(), $player->getPosition(), $this->getFacingDegrees());

		$dyeColor = $item instanceof Dye ? $item->getColor() : match($item->getTypeId()){
			ItemTypeIds::BONE_MEAL => DyeColor::WHITE,
			ItemTypeIds::LAPIS_LAZULI => DyeColor::BLUE,
			ItemTypeIds::COCOA_BEANS => DyeColor::BROWN,
			default => null
		};
		if($dyeColor !== null){
			$color = $dyeColor === DyeColor::BLACK ? new Color(0, 0, 0) : $dyeColor->getRgbValue();
			$text = $this->getFaceText($frontFace);
			if(
				$color->toARGB() !== $text->getBaseColor()->toARGB() &&
				$this->doSignChange(new SignText($text->getLines(), $color, $text->isGlowing()), $player, $item, $frontFace)
			){
				$this->position->getWorld()->addSound($this->position, new DyeUseSound());
				return true;
			}
		}elseif(match($item->getTypeId()){
			ItemTypeIds::INK_SAC => $this->changeSignGlowingState(false, $player, $item, $frontFace),
			ItemTypeIds::GLOW_INK_SAC => $this->changeSignGlowingState(true, $player, $item, $frontFace),
			ItemTypeIds::HONEYCOMB => $this->wax($player, $item),
			default => false
		}){
			return true;
		}

		$player->openSignEditor($this->position, $frontFace);

		return true;
	}

	private function interactsFront(Vector3 $hitboxCenter, Vector3 $playerPosition, float $signFacingDegrees) : bool{
		$playerCenterDiffX = $playerPosition->x - $hitboxCenter->x;
		$playerCenterDiffZ = $playerPosition->z - $hitboxCenter->z;

		$f1 = rad2deg(atan2($playerCenterDiffZ, $playerCenterDiffX)) - 90.0;

		$rotationDiff = $signFacingDegrees - $f1;
		$rotation = fmod($rotationDiff + 180.0, 360.0) - 180.0; // Normalize to [-180, 180]
		return abs($rotation) <= 90.0;
	}

	protected function getHitboxCenter() : Vector3{
		return $this->position->add(0.5, 0.5, 0.5);
	}

	protected function getFacingDegrees() : float{
		return 0;
	}

	public function getText() : SignText{
		return $this->text;
	}

	public function setText(SignText $text) : self{
		$this->text = $text;
		return $this;
	}

	public function getFaceText(bool $frontFace) : SignText{
		return $frontFace ? $this->text : $this->backText;
	}

	public function setFaceText(bool $frontFace, SignText $text) : self{
		$frontFace ? $this->text = $text : $this->backText = $text;
		return $this;
	}

	public function isWaxed() : bool{ return $this->waxed; }

	public function setWaxed(bool $waxed) : self{
		$this->waxed = $waxed;
		return $this;
	}

	public function getEditorEntityRuntimeId() : ?int{ return $this->editorEntityRuntimeId; }

	public function setEditorEntityRuntimeId(?int $editorEntityRuntimeId) : self{
		$this->editorEntityRuntimeId = $editorEntityRuntimeId;
		return $this;
	}

	public function updateText(Player $author, SignText $text) : bool{
		return $this->updateFaceText($author, true, $text);
	}

	public function updateFaceText(Player $author, bool $frontFace, SignText $text) : bool{
		$size = 0;
		foreach($text->getLines() as $line){
			$size += strlen($line);
		}
		if($size > 1000){
			throw new \UnexpectedValueException($author->getName() . " tried to write $size bytes of text onto a sign (bigger than max 1000)");
		}
		$oldText = $this->getFaceText($frontFace);
		$ev = new SignChangeEvent($this, $author, new SignText(array_map(function(string $line) : string{
			return TextFormat::clean($line, false);
		}, $text->getLines()), $oldText->getBaseColor(), $oldText->isGlowing()), $frontFace);
		if($this->waxed || $this->editorEntityRuntimeId !== $author->getId()){
			$ev->cancel();
		}
		$ev->call();
		if(!$ev->isCancelled()){
			$this->setFaceText($frontFace, $ev->getNewText());
			$this->setEditorEntityRuntimeId(null);
			$this->position->getWorld()->setBlock($this->position, $this);
			return true;
		}

		return false;
	}

	public function asItem() : Item{
		return ($this->asItemCallback)();
	}

	public function getFuelTime() : int{
		return $this->woodType->isFlammable() ? 200 : 0;
	}
}
}

namespace pocketmine\block {
use pocketmine\data\runtime\RuntimeDataDescriber;

class Bedrock extends Opaque{
	private bool $burnsForever = false;

	protected function describeBlockOnlyState(RuntimeDataDescriber $w) : void{
		$w->bool($this->burnsForever);
	}

	public function burnsForever() : bool{
		return $this->burnsForever;
	}

	public function setBurnsForever(bool $burnsForever) : self{
		$this->burnsForever = $burnsForever;
		return $this;
	}
}
}

namespace pocketmine\block {
use pocketmine\item\Item;

class BlueIce extends Opaque{

	public function getFrictionFactor() : float{
		return 0.99;
	}

	public function getDropsForCompatibleTool(Item $item) : array{
		return [];
	}

	public function isAffectedBySilkTouch() : bool{
		return true;
	}
}
}

namespace pocketmine\block {
use pocketmine\block\utils\PillarRotation;
use pocketmine\block\utils\PillarRotationTrait;

class BoneBlock extends Opaque implements PillarRotation{
	use PillarRotationTrait;
}
}

namespace pocketmine\block {
use pocketmine\item\Item;
use pocketmine\item\VanillaItems;

class Bookshelf extends Opaque{

	public function getDropsForCompatibleTool(Item $item) : array{
		return [
			VanillaItems::BOOK()->setCount(3)
		];
	}

	public function isAffectedBySilkTouch() : bool{
		return true;
	}

	public function getFuelTime() : int{
		return 300;
	}

	public function getFlameEncouragement() : int{
		return 30;
	}

	public function getFlammability() : int{
		return 20;
	}
}
}

namespace pocketmine\block {
use pocketmine\block\utils\AmethystTrait;
use pocketmine\block\utils\BlockEventHelper;
use pocketmine\item\Item;
use pocketmine\math\Facing;
use function array_rand;
use function mt_rand;

final class BuddingAmethyst extends Opaque{
	use AmethystTrait;

	public function ticksRandomly() : bool{
		return true;
	}

	public function onRandomTick() : void{
		if(mt_rand(1, 5) === 1){
			$face = Facing::ALL[array_rand(Facing::ALL)];

			$adjacent = $this->getSide($face);

			if(!$this->position->getWorld()->isInWorld($adjacent->position->getFloorX(), $adjacent->position->getFloorY(), $adjacent->position->getFloorZ())){
				return;
			}

			//TODO: amethyst buds can spawn in water - we need waterlogging support for this

			$newStage = null;

			if($adjacent->getTypeId() === BlockTypeIds::AIR){
				$newStage = AmethystCluster::STAGE_SMALL_BUD;
			}elseif(
				$adjacent->getTypeId() === BlockTypeIds::AMETHYST_CLUSTER &&
				$adjacent instanceof AmethystCluster &&
				$adjacent->getStage() < AmethystCluster::STAGE_CLUSTER &&
				$adjacent->getFacing() === $face
			){
				$newStage = $adjacent->getStage() + 1;
			}
			if($newStage !== null){
				BlockEventHelper::grow($adjacent, VanillaBlocks::AMETHYST_CLUSTER()->setStage($newStage)->setFacing($face), null);
			}
		}
	}

	public function getDropsForCompatibleTool(Item $item) : array{
		return [];
	}
}
}

namespace pocketmine\block {
use pocketmine\data\runtime\RuntimeDataDescriber;
use pocketmine\item\Item;
use pocketmine\item\ItemBlock;
use pocketmine\math\AxisAlignedBB;
use pocketmine\math\Facing;
use pocketmine\math\Vector3;
use pocketmine\player\Player;

class Cake extends BaseCake{
	public const MAX_BITES = 6;

	protected int $bites = 0;

	protected function describeBlockOnlyState(RuntimeDataDescriber $w) : void{
		$w->boundedIntAuto(0, self::MAX_BITES, $this->bites);
	}

	protected function recalculateCollisionBoxes() : array{
		return [
			AxisAlignedBB::one()
				->contract(1 / 16, 0, 1 / 16)
				->trim(Facing::UP, 0.5)
				->trim(Facing::WEST, $this->bites / 8)
		];
	}

	public function getBites() : int{ return $this->bites; }

	public function setBites(int $bites) : self{
		if($bites < 0 || $bites > self::MAX_BITES){
			throw new \InvalidArgumentException("Bites must be in range 0 ... " . self::MAX_BITES);
		}
		$this->bites = $bites;
		return $this;
	}

	public function onInteract(Item $item, int $face, Vector3 $clickVector, ?Player $player = null, array &$returnedItems = []) : bool{
		if($this->bites === 0 && $item instanceof ItemBlock){
			$block = $item->getBlock();
			$resultBlock = null;
			if($block->getTypeId() === BlockTypeIds::CANDLE){
				$resultBlock = VanillaBlocks::CAKE_WITH_CANDLE();
			}elseif($block instanceof DyedCandle){
				$resultBlock = VanillaBlocks::CAKE_WITH_DYED_CANDLE()->setColor($block->getColor());
			}

			if($resultBlock !== null){
				$this->position->getWorld()->setBlock($this->position, $resultBlock);
				$item->pop();
				return true;
			}
		}

		return parent::onInteract($item, $face, $clickVector, $player, $returnedItems);
	}

	public function getDropsForCompatibleTool(Item $item) : array{
		return [];
	}

	public function getResidue() : Block{
		$clone = clone $this;
		$clone->bites++;
		if($clone->bites > self::MAX_BITES){
			$clone = VanillaBlocks::AIR();
		}
		return $clone;
	}
}
}

namespace pocketmine\block {
use pocketmine\block\utils\CandleTrait;
use pocketmine\block\utils\Lightable;
use pocketmine\entity\Living;
use pocketmine\item\Item;
use pocketmine\math\AxisAlignedBB;
use pocketmine\math\Facing;
use pocketmine\math\Vector3;
use pocketmine\player\Player;

class CakeWithCandle extends BaseCake implements Lightable{
	use CandleTrait {
		onInteract as onInteractCandle;
	}

	protected function recalculateCollisionBoxes() : array{
		return [
			AxisAlignedBB::one()
				->contract(1 / 16, 0, 1 / 16)
				->trim(Facing::UP, 0.5) //TODO: not sure if the candle affects height
		];
	}

	public function getCandle() : Candle{
		return VanillaBlocks::CANDLE();
	}

	public function onInteract(Item $item, int $face, Vector3 $clickVector, ?Player $player = null, array &$returnedItems = []) : bool{
		if($this->lit && $face !== Facing::UP){
			return true;
		}
		if($this->onInteractCandle($item, $face, $clickVector, $player, $returnedItems)){
			return true;
		}

		return parent::onInteract($item, $face, $clickVector, $player, $returnedItems);
	}

	public function getDropsForCompatibleTool(Item $item) : array{
		return [$this->getCandle()->asItem()];
	}

	public function getPickedItem(bool $addUserData = false) : Item{
		return VanillaBlocks::CAKE()->asItem();
	}

	public function getResidue() : Block{
		return VanillaBlocks::CAKE()->setBites(1);
	}

	public function onConsume(Living $consumer) : void{
		parent::onConsume($consumer);
		$this->position->getWorld()->dropItem($this->position->add(0.5, 0.5, 0.5), $this->getCandle()->asItem());
	}
}
}

namespace pocketmine\block {
use pocketmine\block\utils\Colored;
use pocketmine\block\utils\ColoredTrait;
use pocketmine\block\utils\DyeColor;

class CakeWithDyedCandle extends CakeWithCandle implements Colored{
	use ColoredTrait;

	public function __construct(BlockIdentifier $idInfo, string $name, BlockTypeInfo $typeInfo){
		$this->color = DyeColor::WHITE;
		parent::__construct($idInfo, $name, $typeInfo);
	}

	public function getCandle() : Candle{
		return VanillaBlocks::DYED_CANDLE()->setColor($this->color);
	}
}
}

namespace pocketmine\block {
use pocketmine\block\utils\CandleTrait;
use pocketmine\block\utils\Lightable;
use pocketmine\block\utils\SupportType;
use pocketmine\data\runtime\RuntimeDataDescriber;
use pocketmine\item\Item;
use pocketmine\math\Axis;
use pocketmine\math\AxisAlignedBB;
use pocketmine\math\Facing;
use pocketmine\math\Vector3;
use pocketmine\player\Player;
use pocketmine\utils\AssumptionFailedError;
use pocketmine\world\BlockTransaction;

class Candle extends Transparent implements Lightable{
	use CandleTrait {
		describeBlockOnlyState as encodeLitState;
		getLightLevel as getBaseLightLevel;
	}

	public const MIN_COUNT = 1;
	public const MAX_COUNT = 4;

	private int $count = self::MIN_COUNT;

	protected function describeBlockOnlyState(RuntimeDataDescriber $w) : void{
		$this->encodeLitState($w);
		$w->boundedIntAuto(self::MIN_COUNT, self::MAX_COUNT, $this->count);
	}

	public function getCount() : int{ return $this->count; }

	public function setCount(int $count) : self{
		if($count < self::MIN_COUNT || $count > self::MAX_COUNT){
			throw new \InvalidArgumentException("Count must be in range " . self::MIN_COUNT . " ... " . self::MAX_COUNT);
		}
		$this->count = $count;
		return $this;
	}

	public function getLightLevel() : int{
		return $this->getBaseLightLevel() * $this->count;
	}

	protected function recalculateCollisionBoxes() : array{
		return [
			(match($this->count){
				1 => AxisAlignedBB::one()
					->squash(Axis::X, 7 / 16)
					->squash(Axis::Z, 7 / 16),
				2 => AxisAlignedBB::one()
					->squash(Axis::X, 5 / 16)
					->trim(Facing::NORTH, 7 / 16) //0.3 thick on the Z axis
					->trim(Facing::SOUTH, 6 / 16),
				3 => AxisAlignedBB::one()
					->trim(Facing::WEST, 5 / 16)
					->trim(Facing::EAST, 6 / 16)
					->trim(Facing::NORTH, 6 / 16)
					->trim(Facing::SOUTH, 5 / 16),
				4 => AxisAlignedBB::one()
					->squash(Axis::X, 5 / 16)
					->trim(Facing::NORTH, 5 / 16)
					->trim(Facing::SOUTH, 6 / 16),
				default => throw new AssumptionFailedError("Unreachable")
			})->trim(Facing::UP, 10 / 16)
		];
	}

	public function getSupportType(int $facing) : SupportType{
		return SupportType::NONE;
	}

	protected function getCandleIfCompatibleType(Block $block) : ?Candle{
		return $block instanceof Candle && $block->hasSameTypeId($this) ? $block : null;
	}

	public function canBePlacedAt(Block $blockReplace, Vector3 $clickVector, int $face, bool $isClickedBlock) : bool{
		$candle = $this->getCandleIfCompatibleType($blockReplace);
		return $candle !== null ? $candle->count < self::MAX_COUNT : parent::canBePlacedAt($blockReplace, $clickVector, $face, $isClickedBlock);
	}

	public function place(BlockTransaction $tx, Item $item, Block $blockReplace, Block $blockClicked, int $face, Vector3 $clickVector, ?Player $player = null) : bool{
		if(!$blockReplace->getAdjacentSupportType(Facing::DOWN)->hasCenterSupport()){
			return false;
		}
		$existing = $this->getCandleIfCompatibleType($blockReplace);
		if($existing !== null){
			if($existing->count >= self::MAX_COUNT){
				return false;
			}

			$this->count = $existing->count + 1;
			$this->lit = $existing->lit;
		}
		return parent::place($tx, $item, $blockReplace, $blockClicked, $face, $clickVector, $player);
	}

	public function getDropsForCompatibleTool(Item $item) : array{
		return [$this->asItem()->setCount($this->count)];
	}
}
}

namespace pocketmine\block {
use pocketmine\block\inventory\CartographyTableInventory;
use pocketmine\item\Item;
use pocketmine\math\Vector3;
use pocketmine\player\Player;

final class CartographyTable extends Opaque{

	public function onInteract(Item $item, int $face, Vector3 $clickVector, ?Player $player = null, array &$returnedItems = []) : bool{
		if($player !== null){
			$player->setCurrentWindow(new CartographyTableInventory($this->position));
		}

		return true;
	}

	public function getFuelTime() : int{
		return 300;
	}
}
}

namespace pocketmine\block {
use pocketmine\block\utils\FacesOppositePlacingPlayerTrait;
use pocketmine\block\utils\HorizontalFacing;

class CarvedPumpkin extends Opaque implements HorizontalFacing{
	use FacesOppositePlacingPlayerTrait;
}
}

namespace pocketmine\block {
use pocketmine\block\utils\SignLikeRotation;
use pocketmine\block\utils\SignLikeRotationTrait;
use pocketmine\block\utils\StaticSupportTrait;
use pocketmine\item\Item;
use pocketmine\math\Facing;
use pocketmine\math\Vector3;
use pocketmine\player\Player;
use pocketmine\world\BlockTransaction;

final class CeilingCenterHangingSign extends BaseSign implements SignLikeRotation{
	use SignLikeRotationTrait;
	use StaticSupportTrait;

	protected function getSupportingFace() : int{
		return Facing::UP;
	}

	//TODO: duplicated code :(
	public function place(BlockTransaction $tx, Item $item, Block $blockReplace, Block $blockClicked, int $face, Vector3 $clickVector, ?Player $player = null) : bool{
		if($face !== Facing::DOWN){
			return false;
		}

		if($player !== null){
			$this->rotation = self::getRotationFromYaw($player->getLocation()->getYaw());
		}
		return parent::place($tx, $item, $blockReplace, $blockClicked, $face, $clickVector, $player);
	}

	private function canBeSupportedAt(Block $block) : bool{
		$supportBlock = $block->getSide(Facing::UP);
		return
			$supportBlock->getSupportType(Facing::DOWN)->hasCenterSupport() ||
			$supportBlock->hasTypeTag(BlockTypeTags::HANGING_SIGN);
	}

	protected function getFacingDegrees() : float{
		return $this->rotation * 22.5;
	}
}
}

namespace pocketmine\block {
use pocketmine\block\utils\HorizontalFacing;
use pocketmine\block\utils\HorizontalFacingTrait;
use pocketmine\block\utils\SupportType;
use pocketmine\item\Item;
use pocketmine\math\Facing;
use pocketmine\math\Vector3;
use pocketmine\player\Player;
use pocketmine\utils\AssumptionFailedError;
use pocketmine\world\BlockTransaction;

final class CeilingEdgesHangingSign extends BaseSign implements HorizontalFacing{
	use HorizontalFacingTrait;

	protected function getSupportingFace() : int{
		return Facing::UP;
	}

	public function place(BlockTransaction $tx, Item $item, Block $blockReplace, Block $blockClicked, int $face, Vector3 $clickVector, ?Player $player = null) : bool{
		if($face !== Facing::DOWN){
			return false;
		}
		if($player !== null){
			$this->facing = Facing::opposite($player->getHorizontalFacing());
		}
		if(!$this->canBeSupportedAt($blockReplace)){
			return false;
		}

		return parent::place($tx, $item, $blockReplace, $blockClicked, $face, $clickVector, $player);
	}

	public function onNearbyBlockChange() : void{
		if(!$this->canBeSupportedAt($this)){
			$this->position->getWorld()->useBreakOn($this->position);
		}
	}

	private function canBeSupportedAt(Block $block) : bool{
		$supportBlock = $block->getSide(Facing::UP);
		return
			$supportBlock->getSupportType(Facing::DOWN) === SupportType::FULL ||
			(($supportBlock instanceof WallHangingSign || $supportBlock instanceof CeilingEdgesHangingSign) && Facing::axis($supportBlock->getFacing()) === Facing::axis($this->facing));
	}

	protected function getFacingDegrees() : float{
		return match($this->facing){
			Facing::SOUTH => 0,
			Facing::WEST => 90,
			Facing::NORTH => 180,
			Facing::EAST => 270,
			default => throw new AssumptionFailedError("Invalid facing direction: " . $this->facing),
		};
	}
}
}

namespace pocketmine\block {
final class ChemicalHeat extends Transparent{

	}
}

namespace pocketmine\block {
use pocketmine\block\utils\FacesOppositePlacingPlayerTrait;
use pocketmine\block\utils\HorizontalFacing;
use pocketmine\item\Item;
use pocketmine\math\Vector3;
use pocketmine\player\Player;

final class ChemistryTable extends Opaque implements HorizontalFacing{
	use FacesOppositePlacingPlayerTrait;

	public function onInteract(Item $item, int $face, Vector3 $clickVector, ?Player $player = null, array &$returnedItems = []) : bool{
		//TODO
		return false;
	}
}
}

namespace pocketmine\block {
use pocketmine\item\Item;
use pocketmine\item\VanillaItems;

class Clay extends Opaque{

	public function getDropsForCompatibleTool(Item $item) : array{
		return [
			VanillaItems::CLAY()->setCount(4)
		];
	}

	public function isAffectedBySilkTouch() : bool{
		return true;
	}
}
}

namespace pocketmine\block {
class Coal extends Opaque{

	public function getFuelTime() : int{
		return 16000;
	}

	public function getFlameEncouragement() : int{
		return 5;
	}

	public function getFlammability() : int{
		return 5;
	}
}
}

namespace pocketmine\block {
use pocketmine\block\utils\FortuneDropHelper;
use pocketmine\item\Item;
use pocketmine\item\VanillaItems;
use function mt_rand;

class CoalOre extends Opaque{

	public function getDropsForCompatibleTool(Item $item) : array{
		return [
			VanillaItems::COAL()->setCount(FortuneDropHelper::weighted($item, min: 1, maxBase: 1))
		];
	}

	public function isAffectedBySilkTouch() : bool{
		return true;
	}

	protected function getXpDropAmount() : int{
		return mt_rand(0, 2);
	}
}
}

namespace pocketmine\block {
use pocketmine\block\utils\Colored;
use pocketmine\block\utils\ColoredTrait;

class Concrete extends Opaque implements Colored{
	use ColoredTrait;
}
}

namespace pocketmine\block {
use pocketmine\block\utils\BlockEventHelper;
use pocketmine\block\utils\Colored;
use pocketmine\block\utils\ColoredTrait;
use pocketmine\block\utils\Fallable;
use pocketmine\block\utils\FallableTrait;
use pocketmine\math\Facing;

class ConcretePowder extends Opaque implements Fallable, Colored{
	use ColoredTrait;
	use FallableTrait {
		onNearbyBlockChange as protected startFalling;
	}

	public function onNearbyBlockChange() : void{
		if(($water = $this->getAdjacentWater()) !== null){
			BlockEventHelper::form($this, VanillaBlocks::CONCRETE()->setColor($this->color), $water);
		}else{
			$this->startFalling();
		}
	}

	public function tickFalling() : ?Block{
		if($this->getAdjacentWater() === null){
			return null;
		}
		return VanillaBlocks::CONCRETE()->setColor($this->color);
	}

	private function getAdjacentWater() : ?Water{
		foreach(Facing::ALL as $i){
			if($i === Facing::DOWN){
				continue;
			}
			$block = $this->getSide($i);
			if($block instanceof Water){
				return $block;
			}
		}

		return null;
	}
}
}

namespace pocketmine\block {
use pocketmine\math\Facing;

class PotentSulfur extends Opaque{

	public function onNearbyBlockChange() : void{
		$result = $this->computeVariant();
		if($result->getTypeId() !== $this->getTypeId()){
			$world = $this->position->getWorld();
			$world->setBlock($this->position, $result);

			//TODO: HACK! World operations cannot be performed on the instance just
			//created here because it lacks a position, and World::setBlock()
			//clones the passed instance before placing it in the world.
			$applied = $world->getBlock($this->position);
			if($applied instanceof PotentSulfur){
				$applied->onGeyserVariantApplied();
			}
		}
	}

	protected function onGeyserVariantApplied() : void{
		//NOOP
	}

	private function computeVariant() : PotentSulfur{
		$above = $this->getSide(Facing::UP);
		if(!($above instanceof Water) || !$above->isSource()){ //TODO: waterlogging!
			return VanillaBlocks::POTENT_SULFUR();
		}

		$below = $this->getSide(Facing::DOWN);
		if($below instanceof Lava && $below->isSource()){
			return VanillaBlocks::CONTINUOUS_POTENT_SULFUR();
		}

		if($below->getTypeId() === BlockTypeIds::MAGMA){
			return VanillaBlocks::CYCLING_POTENT_SULFUR();
		}

		return VanillaBlocks::WET_POTENT_SULFUR();
	}
}
}

namespace pocketmine\block {
use pocketmine\entity\effect\EffectInstance;
use pocketmine\entity\effect\VanillaEffects;
use pocketmine\entity\Living;
use pocketmine\item\Item;
use pocketmine\math\AxisAlignedBB;
use pocketmine\math\Facing;
use pocketmine\math\Vector3;
use pocketmine\math\VoxelRayTrace;
use function count;

class WetPotentSulfur extends PotentSulfur{

	private const HEARTBEAT_TICKS = 10;

	private const MAX_COLUMN_HEIGHT_BLOCKS = 4;

	private const GAS_EFFECT_DURATION_TICKS = 80; //4 seconds
	private const GAS_EFFECT_RANGE_BLOCKS = 3.0;

	public function asItem() : Item{
		return VanillaBlocks::POTENT_SULFUR()->asItem();
	}

	public function onScheduledUpdate() : void{
		$this->onGeyserHeartbeat($this->findGeyserOutlet());
		$this->position->getWorld()->scheduleDelayedBlockUpdate($this->position, self::HEARTBEAT_TICKS);
	}

	protected function onGeyserVariantApplied() : void{
		//A dry block doesn't tick, so its heartbeat has died by the time it becomes a geyser and has to be started again
		//here. Scheduling is idempotent at this delay, so doing it for a variant that is already beating costs nothing.
		$this->position->getWorld()->scheduleDelayedBlockUpdate($this->position, self::HEARTBEAT_TICKS);
	}

	protected function onGeyserHeartbeat(?Block $outlet) : void{
		if($outlet !== null){
			$this->pulseGas($outlet);
		}
	}

	protected function isGeyserPassable(Block $block) : bool{
		return count($block->getCollisionBoxes()) === 0;
	}

	protected function getWaterColumnLength(Block $outlet) : int{
		return $outlet->position->getFloorY() - $this->position->getFloorY() - 1;
	}

	protected function findGeyserOutlet() : ?Block{
		$maxY = $this->position->getFloorY() + self::MAX_COLUMN_HEIGHT_BLOCKS + 1;

		$block = $this->getSide(Facing::UP);
		while($block->position->getFloorY() <= $maxY){
			if($block instanceof Water && $block->isSource()){
				$block = $block->getSide(Facing::UP);
				continue;
			}

			return $this->isGeyserPassable($block) ? $block : null;
		}

		return null;
	}

	private function pulseGas(Block $outlet) : void{
		$world = $this->position->getWorld();
		$bb = AxisAlignedBB::one()
			->offset($outlet->position->x, $outlet->position->y, $outlet->position->z)
			->expand(2.5, 0.0, 2.5);

		foreach($world->getCollidingEntities($bb) as $entity){
			if(!$entity instanceof Living){
				continue;
			}

			if($this->isExposedToGas($outlet, $entity->getEyePos())){
				$entity->getEffects()->add(new EffectInstance(VanillaEffects::NAUSEA(), self::GAS_EFFECT_DURATION_TICKS, 0, true, true));
			}
		}
	}

	private function isExposedToGas(Block $outlet, Vector3 $eyePos) : bool{
		$world = $this->position->getWorld();
		if(!$this->isGeyserPassable($world->getBlock($eyePos))){
			return false;
		}
		if($eyePos->distanceSquared($outlet->position->add(0.5, 0.5, 0.5)) > self::GAS_EFFECT_RANGE_BLOCKS ** 2){
			return false;
		}

		$feetPos = new Vector3($eyePos->x, $eyePos->y - 1, $eyePos->z);
		$feetBlock = $world->getBlock($feetPos);
		if(!($feetBlock instanceof Water) || !$feetBlock->isSource()){
			return false;
		}

		$center = $outlet->position->add(0.5, -0.5, 0.5);
		foreach(VoxelRayTrace::betweenPoints($center, $feetPos) as $blockPos){
			$block = $world->getBlockAt($blockPos->getFloorX(), $blockPos->getFloorY(), $blockPos->getFloorZ());
			foreach($block->getCollisionBoxes() as $bb){
				if($bb->calculateIntercept($center, $feetPos) !== null){
					return false;
				}
			}
		}

		return true;
	}
}
}

namespace pocketmine\block {
use pocketmine\event\block\GeyserEruptionPulseEvent;
use pocketmine\math\AxisAlignedBB;
use pocketmine\math\Facing;
use pocketmine\player\Player;
use pocketmine\world\sound\Sound;
use function mt_rand;

abstract class EruptivePotentSulfur extends WetPotentSulfur{

	private const ERUPTION_SOUND_INTERVAL_HEARTBEATS = 4; //2 seconds

	private const BASE_LAUNCH_SPEED = 0.3;
	private const LAUNCH_FORCE = 0.2;

	private const PLUME_BLOCKS_PER_DEPTH = 6;

	abstract public function isErupting() : bool;
	abstract public function getEruptionStartSound() : Sound;
	abstract public function getEruptionBurstSound() : Sound;

	protected function onGeyserHeartbeat(?Block $outlet) : void{
		parent::onGeyserHeartbeat($outlet);

		if($outlet !== null && $this->isErupting()){
			$cancelled = false;
			if(GeyserEruptionPulseEvent::hasHandlers()){
				$ev = new GeyserEruptionPulseEvent($this, $outlet);
				$ev->call();
				$cancelled = $ev->isCancelled();
			}

			if(!$cancelled){
				$this->pulseEruption($outlet);
			}
		}
	}

	private function getPlumeHeight(int $columnDepth) : int{
		$maxHeight = self::PLUME_BLOCKS_PER_DEPTH * $columnDepth;

		for($i = 0; $i < $maxHeight; $i++){
			if(!$this->isGeyserPassable($this->getSide(Facing::UP, $i + 1))){
				return $i;
			}
		}

		return $maxHeight;
	}

	private function pulseEruption(Block $outlet) : void{
		$this->pulsePush($outlet);

		if(mt_rand(1, self::ERUPTION_SOUND_INTERVAL_HEARTBEATS) === 1){
			$this->position->getWorld()->addSound($outlet->position->add(0.5, 0.5, 0.5), $this->getEruptionBurstSound());
		}
	}

	private function pulsePush(Block $outlet) : void{
		$columnLength = $this->getWaterColumnLength($outlet);
		$plumeHeight = $this->getPlumeHeight($columnLength);
		if($plumeHeight <= 0){
			return;
		}

		$bb = AxisAlignedBB::one()
			->offset($this->position->x, $this->position->y + 1, $this->position->z)
			->extend(Facing::UP, $plumeHeight - 1);
		$maxSpeed = self::BASE_LAUNCH_SPEED + $columnLength * 0.1;

		foreach($this->position->getWorld()->getCollidingEntities($bb) as $entity){
			if($entity instanceof Player && $entity->isFlying()){
				continue;
			}

			$entity->resetFallDistance();
			if($entity->getMotion()->y < $maxSpeed){
				$entity->addMotion(0, self::LAUNCH_FORCE, 0);
			}
		}
	}
}
}

namespace pocketmine\block {
use pocketmine\world\sound\GeyserContinuousEruptionBurstSound;
use pocketmine\world\sound\GeyserContinuousEruptionStartSound;
use pocketmine\world\sound\Sound;

final class ContinuousPotentSulfur extends EruptivePotentSulfur{

	public function isErupting() : bool{
		return $this->findGeyserOutlet() !== null;
	}

	protected function onGeyserVariantApplied() : void{
		parent::onGeyserVariantApplied();
		$this->position->getWorld()->addSound($this->position->add(0.5, 0.5, 0.5), $this->getEruptionStartSound());
	}

	public function getEruptionStartSound() : Sound{
		return new GeyserContinuousEruptionStartSound();
	}

	public function getEruptionBurstSound() : Sound{
		return new GeyserContinuousEruptionBurstSound();
	}
}
}

namespace pocketmine\block {
use pocketmine\block\utils\CopperMaterial;
use pocketmine\block\utils\CopperTrait;

class Copper extends Opaque implements CopperMaterial{
	use CopperTrait;
}
}

namespace pocketmine\block {
use pocketmine\block\utils\CopperMaterial;
use pocketmine\block\utils\CopperOxidation;
use pocketmine\block\utils\CopperTrait;
use pocketmine\block\utils\Lightable;
use pocketmine\block\utils\LightableTrait;
use pocketmine\block\utils\PoweredByRedstone;
use pocketmine\block\utils\PoweredByRedstoneTrait;
use pocketmine\data\runtime\RuntimeDataDescriber;

class CopperBulb extends Opaque implements CopperMaterial, Lightable, PoweredByRedstone{
	use CopperTrait;
	use PoweredByRedstoneTrait;
	use LightableTrait{
		describeBlockOnlyState as encodeLitState;
	}

	protected function describeBlockOnlyState(RuntimeDataDescriber $w) : void{
		$this->encodeLitState($w);
		$w->bool($this->powered);
	}

	public function togglePowered(bool $powered) : self{
		if($powered === $this->powered){
			return $this;
		}
		if ($powered) {
			$this->setLit(!$this->lit);
		}
		$this->setPowered($powered);
		return $this;
	}

	public function getLightLevel() : int{
		if ($this->lit) {
			return match($this->oxidation){
				CopperOxidation::NONE => 15,
				CopperOxidation::EXPOSED => 12,
				CopperOxidation::WEATHERED => 8,
				CopperOxidation::OXIDIZED => 4,
			};
		}

		return 0;
	}
}
}

namespace pocketmine\block {
use pocketmine\block\utils\CopperMaterial;
use pocketmine\block\utils\CopperTrait;
use pocketmine\item\Item;
use pocketmine\math\Facing;
use pocketmine\math\Vector3;
use pocketmine\player\Player;

class CopperDoor extends Door implements CopperMaterial{
	use CopperTrait{
		onInteract as onInteractCopper;
	}

	public function onInteract(Item $item, int $face, Vector3 $clickVector, ?Player $player = null, array &$returnedItems = []) : bool{
		if ($player !== null && $player->isSneaking() && $this->onInteractCopper($item, $face, $clickVector, $player, $returnedItems)) {
			//copy copper properties to other half
			$other = $this->getSide($this->top ? Facing::DOWN : Facing::UP);
			$world = $this->position->getWorld();
			if ($other instanceof CopperDoor) {
				$other->setOxidation($this->oxidation);
				$other->setWaxed($this->waxed);
				$world->setBlock($other->position, $other);
			}
			return true;
		}

		return parent::onInteract($item, $face, $clickVector, $player, $returnedItems);
	}
}
}

namespace pocketmine\block {
use pocketmine\block\utils\SupportType;
use pocketmine\data\runtime\RuntimeDataDescriber;
use pocketmine\item\Item;
use pocketmine\math\Axis;
use pocketmine\math\AxisAlignedBB;
use pocketmine\math\Facing;
use pocketmine\math\Vector3;
use pocketmine\player\Player;
use pocketmine\world\BlockTransaction;

class Lantern extends Transparent{
	private int $lightLevel; //readonly

	protected bool $hanging = false;

	public function __construct(BlockIdentifier $idInfo, string $name, BlockTypeInfo $typeInfo, int $lightLevel){
		$this->lightLevel = $lightLevel;
		parent::__construct($idInfo, $name, $typeInfo);
	}

	protected function describeBlockOnlyState(RuntimeDataDescriber $w) : void{
		$w->bool($this->hanging);
	}

	public function isHanging() : bool{ return $this->hanging; }

	public function setHanging(bool $hanging) : self{
		$this->hanging = $hanging;
		return $this;
	}

	public function getLightLevel() : int{
		return $this->lightLevel;
	}

	protected function recalculateCollisionBoxes() : array{
		return [
			AxisAlignedBB::one()
				->trim(Facing::UP,   $this->hanging ? 6 / 16 : 8 / 16)
				->trim(Facing::DOWN, $this->hanging ? 2 / 16 : 0)
				->squash(Axis::X, 5 / 16)
				->squash(Axis::Z, 5 / 16)
		];
	}

	public function getSupportType(int $facing) : SupportType{
		return SupportType::NONE;
	}

	public function place(BlockTransaction $tx, Item $item, Block $blockReplace, Block $blockClicked, int $face, Vector3 $clickVector, ?Player $player = null) : bool{
		$downSupport = $this->canBeSupportedAt($blockReplace, Facing::DOWN);
		if(!$downSupport && !$this->canBeSupportedAt($blockReplace, Facing::UP)){
			return false;
		}

		$this->hanging = $face === Facing::DOWN || !$downSupport;
		return parent::place($tx, $item, $blockReplace, $blockClicked, $face, $clickVector, $player);
	}

	public function onNearbyBlockChange() : void{
		$face = $this->hanging ? Facing::UP : Facing::DOWN;
		if(!$this->canBeSupportedAt($this, $face)){
			$this->position->getWorld()->useBreakOn($this->position);
		}
	}

	private function canBeSupportedAt(Block $block, int $face) : bool{
		return $block->getAdjacentSupportType($face)->hasCenterSupport();
	}
}
}

namespace pocketmine\block {
use pocketmine\block\utils\CopperMaterial;
use pocketmine\block\utils\CopperTrait;

class CopperLantern extends Lantern implements CopperMaterial{
	use CopperTrait;
}
}

namespace pocketmine\block {
use pocketmine\block\utils\FortuneDropHelper;
use pocketmine\item\Item;
use pocketmine\item\VanillaItems;

final class CopperOre extends Opaque{

	public function getDropsForCompatibleTool(Item $item) : array{
		return [
			VanillaItems::RAW_COPPER()->setCount(FortuneDropHelper::weighted($item, min: 2, maxBase: 5)),
		];
	}

	public function isAffectedBySilkTouch() : bool{ return true; }
}
}

namespace pocketmine\block {
use pocketmine\block\utils\CopperMaterial;
use pocketmine\block\utils\CopperTrait;
use pocketmine\item\Item;
use pocketmine\math\Vector3;
use pocketmine\player\Player;

class CopperTrapdoor extends Trapdoor implements CopperMaterial{
	use CopperTrait{
		onInteract as onInteractCopper;
	}

	public function onInteract(Item $item, int $face, Vector3 $clickVector, ?Player $player = null, array &$returnedItems = []) : bool{
		if ($player !== null && $player->isSneaking() && $this->onInteractCopper($item, $face, $clickVector, $player, $returnedItems)) {
			return true;
		}

		return parent::onInteract($item, $face, $clickVector, $player, $returnedItems);
	}
}
}

namespace pocketmine\block {
use pocketmine\block\utils\StaticSupportTrait;
use pocketmine\math\Facing;

final class Coral extends BaseCoral{
	use StaticSupportTrait;

	private function canBeSupportedAt(Block $block) : bool{
		return $block->getAdjacentSupportType(Facing::DOWN)->hasCenterSupport();
	}
}
}

namespace pocketmine\block {
use pocketmine\block\utils\BlockEventHelper;
use pocketmine\block\utils\CoralMaterial;
use pocketmine\block\utils\CoralTypeTrait;
use pocketmine\item\Item;
use function mt_rand;

final class CoralBlock extends Opaque implements CoralMaterial{
	use CoralTypeTrait;

	public function onNearbyBlockChange() : void{
		if(!$this->dead){
			$this->position->getWorld()->scheduleDelayedBlockUpdate($this->position, mt_rand(40, 200));
		}
	}

	public function onScheduledUpdate() : void{
		if(!$this->dead){
			$world = $this->position->getWorld();

			$hasWater = false;
			foreach($this->position->sides() as $vector3){
				if($world->getBlock($vector3) instanceof Water){
					$hasWater = true;
					break;
				}
			}
			if(!$hasWater){
				BlockEventHelper::die($this, (clone $this)->setDead(true));
			}
		}
	}

	public function getDropsForCompatibleTool(Item $item) : array{
		return [(clone $this)->setDead(true)->asItem()];
	}

	public function isAffectedBySilkTouch() : bool{
		return true;
	}
}
}

namespace pocketmine\block {
use pocketmine\block\inventory\CraftingTableInventory;
use pocketmine\item\Item;
use pocketmine\math\Vector3;
use pocketmine\player\Player;

class CraftingTable extends Opaque{

	public function onInteract(Item $item, int $face, Vector3 $clickVector, ?Player $player = null, array &$returnedItems = []) : bool{
		if($player instanceof Player){
			$player->setCurrentWindow(new CraftingTableInventory($this->position));
		}

		return true;
	}

	public function getFuelTime() : int{
		return 300;
	}
}
}

namespace pocketmine\block {
use pocketmine\block\tile\PotentSulfur as TilePotentSulfur;
use pocketmine\data\runtime\RuntimeDataDescriber;
use pocketmine\world\sound\GeyserEruptionBurstSound;
use pocketmine\world\sound\GeyserEruptionStartSound;
use pocketmine\world\sound\Sound;
use function max;
use function mt_rand;

final class CyclingPotentSulfur extends EruptivePotentSulfur{

	private const DORMANT_MIN_BASE_HEARTBEATS = 30; //15 seconds
	private const DORMANT_MAX_BASE_HEARTBEATS = 60; //30 seconds
	private const DORMANT_HEARTBEATS_PER_EXTRA_LENGTH = 20; //10 seconds

	private const ERUPTING_MIN_BASE_HEARTBEATS = 2; //1 second
	private const ERUPTING_MAX_BASE_HEARTBEATS = 4; //2 seconds
	private const ERUPTING_HEARTBEATS_PER_EXTRA_LENGTH = 2; //1 second

	private bool $erupting = false;

	private int $heartbeatsUntilPhaseShift = 0;

	protected function describeBlockOnlyState(RuntimeDataDescriber $w) : void{
		$w->bool($this->erupting);
	}

	public function readStateFromWorld() : Block{
		parent::readStateFromWorld();

		$tile = $this->position->getWorld()->getTile($this->position);
		if($tile instanceof TilePotentSulfur){
			$this->heartbeatsUntilPhaseShift = $tile->getHeartbeatsUntilPhaseShift();
		}

		return $this;
	}

	public function writeStateToWorld() : void{
		parent::writeStateToWorld();

		$tile = $this->position->getWorld()->getTile($this->position);
		if($tile instanceof TilePotentSulfur){
			$tile->setHeartbeatsUntilPhaseShift($this->heartbeatsUntilPhaseShift);
		}
	}

	public function isErupting() : bool{ return $this->erupting; }

	public function setErupting(bool $erupting) : self{
		$this->erupting = $erupting;
		return $this;
	}

	public function erupt() : bool{
		if($this->erupting){
			return false;
		}

		$outlet = $this->findGeyserOutlet();
		if($outlet === null){
			return false;
		}

		$this->shiftPhase($outlet);
		$this->heartbeatsUntilPhaseShift = $this->getCurrentPhaseHeartbeats($outlet);
		$this->position->getWorld()->setBlock($this->position, $this);

		return true;
	}

	public function getEruptionStartSound() : Sound{
		return new GeyserEruptionStartSound();
	}

	public function getEruptionBurstSound() : Sound{
		return new GeyserEruptionBurstSound();
	}

	protected function onGeyserHeartbeat(?Block $outlet) : void{
		parent::onGeyserHeartbeat($outlet);

		if($this->heartbeatsUntilPhaseShift <= 0){
			//Set budget before countdown to prevent an instant shift on the first heartbeat,
			//and to avoid immediate eruptions the moment a blocked outlet reopens.
			$this->heartbeatsUntilPhaseShift = $this->getCurrentPhaseHeartbeats($outlet);
		}

		if(--$this->heartbeatsUntilPhaseShift <= 0 && $outlet !== null){
			$this->shiftPhase($outlet);
		}

		$this->position->getWorld()->setBlock($this->position, $this);
	}

	private function shiftPhase(Block $outlet) : void{
		$this->erupting = !$this->erupting;

		if($this->erupting){
			$this->position->getWorld()->addSound($this->position->add(0.5, 0.5, 0.5), $this->getEruptionStartSound());
			$this->position->getWorld()->addSound($outlet->position->add(0.5, 0.5, 0.5), $this->getEruptionBurstSound());
		}
	}

	private function getCurrentPhaseHeartbeats(?Block $outlet) : int{
		$columnLength = $outlet !== null ? max(0, $this->getWaterColumnLength($outlet) - 1) : 0;

		[$min, $max, $extra] = match($this->erupting){
			true => [
				self::ERUPTING_MIN_BASE_HEARTBEATS,
				self::ERUPTING_MAX_BASE_HEARTBEATS,
				self::ERUPTING_HEARTBEATS_PER_EXTRA_LENGTH,
			],
			false => [
				self::DORMANT_MIN_BASE_HEARTBEATS,
				self::DORMANT_MAX_BASE_HEARTBEATS,
				self::DORMANT_HEARTBEATS_PER_EXTRA_LENGTH,
			]
		};

		return mt_rand($min, $max) + ($columnLength * $extra);
	}
}
}

namespace pocketmine\block {
use pocketmine\block\utils\FortuneDropHelper;
use pocketmine\item\Item;
use pocketmine\item\VanillaItems;
use function mt_rand;

class DiamondOre extends Opaque{

	public function getDropsForCompatibleTool(Item $item) : array{
		return [
			VanillaItems::DIAMOND()->setCount(FortuneDropHelper::weighted($item, min: 1, maxBase: 1))
		];
	}

	public function isAffectedBySilkTouch() : bool{
		return true;
	}

	protected function getXpDropAmount() : int{
		return mt_rand(3, 7);
	}
}
}

namespace pocketmine\block {
use pocketmine\block\utils\DirtType;
use pocketmine\data\runtime\RuntimeDataDescriber;
use pocketmine\item\Fertilizer;
use pocketmine\item\Hoe;
use pocketmine\item\Item;
use pocketmine\item\Potion;
use pocketmine\item\PotionType;
use pocketmine\item\SplashPotion;
use pocketmine\math\Facing;
use pocketmine\math\Vector3;
use pocketmine\player\Player;
use pocketmine\world\sound\ItemUseOnBlockSound;
use pocketmine\world\sound\WaterSplashSound;

class Dirt extends Opaque{
	protected DirtType $dirtType = DirtType::NORMAL;

	public function describeBlockItemState(RuntimeDataDescriber $w) : void{
		$w->enum($this->dirtType);
	}

	public function getDirtType() : DirtType{ return $this->dirtType; }

	public function setDirtType(DirtType $dirtType) : self{
		$this->dirtType = $dirtType;
		return $this;
	}

	public function onInteract(Item $item, int $face, Vector3 $clickVector, ?Player $player = null, array &$returnedItems = []) : bool{
		$world = $this->position->getWorld();
		if($face !== Facing::DOWN && $item instanceof Hoe){
			$up = $this->getSide(Facing::UP);
			if($up->getTypeId() !== BlockTypeIds::AIR){
				return true;
			}

			$item->applyDamage(1);

			$newBlock = $this->dirtType === DirtType::NORMAL ? VanillaBlocks::FARMLAND() : VanillaBlocks::DIRT();
			$center = $this->position->add(0.5, 0.5, 0.5);
			$world->addSound($center, new ItemUseOnBlockSound($newBlock));
			$world->setBlock($this->position, $newBlock);
			if($this->dirtType === DirtType::ROOTED){
				$world->dropItem($center, VanillaBlocks::HANGING_ROOTS()->asItem());
			}

			return true;
		}elseif($this->dirtType === DirtType::ROOTED && $item instanceof Fertilizer){
			$down = $this->getSide(Facing::DOWN);
			if($down->getTypeId() !== BlockTypeIds::AIR){
				return true;
			}

			$item->pop();
			$world->setBlock($down->position, VanillaBlocks::HANGING_ROOTS());
			//TODO: bonemeal particles, growth sounds
		}elseif(($item instanceof Potion || $item instanceof SplashPotion) && $item->getType() === PotionType::WATER){
			$item->pop();
			$world->setBlock($this->position, VanillaBlocks::MUD());
			$world->addSound($this->position, new WaterSplashSound(0.5));
			return true;
		}

		return false;
	}
}
}

namespace pocketmine\block {
use pocketmine\block\utils\Fallable;
use pocketmine\block\utils\FallableTrait;
use pocketmine\block\utils\SupportType;
use pocketmine\event\block\BlockTeleportEvent;
use pocketmine\item\Item;
use pocketmine\math\Vector3;
use pocketmine\player\GameMode;
use pocketmine\player\Player;
use pocketmine\world\particle\DragonEggTeleportParticle;
use function max;
use function min;
use function mt_rand;

class DragonEgg extends Transparent implements Fallable{
	use FallableTrait;

	public function getLightLevel() : int{
		return 1;
	}

	public function onInteract(Item $item, int $face, Vector3 $clickVector, ?Player $player = null, array &$returnedItems = []) : bool{
		$this->teleport();
		return true;
	}

	public function onAttack(Item $item, int $face, ?Player $player = null) : bool{
		if($player !== null && $player->getGamemode() !== GameMode::CREATIVE){
			$this->teleport();
			return true;
		}
		return false;
	}

	public function teleport() : void{
		$world = $this->position->getWorld();
		for($tries = 0; $tries < 16; ++$tries){
			$block = $world->getBlockAt(
				$this->position->x + mt_rand(-16, 16),
				max($world->getMinY(), min($world->getMaxY() - 1, $this->position->y + mt_rand(-8, 8))),
				$this->position->z + mt_rand(-16, 16)
			);
			if($block instanceof Air){
				$ev = new BlockTeleportEvent($this, $block->position);
				$ev->call();
				if($ev->isCancelled()){
					break;
				}

				$blockPos = $ev->getTo();
				$world->addParticle($this->position, new DragonEggTeleportParticle($this->position->x - $blockPos->x, $this->position->y - $blockPos->y, $this->position->z - $blockPos->z));
				$world->setBlock($this->position, VanillaBlocks::AIR());
				$world->setBlock($blockPos, $this);
				break;
			}
		}
	}

	public function getSupportType(int $facing) : SupportType{
		return SupportType::NONE;
	}
}
}

namespace pocketmine\block {
class DriedKelp extends Opaque{

	public function getFlameEncouragement() : int{
		return 30;
	}

	public function getFlammability() : int{
		return 60;
	}

	public function getFuelTime() : int{
		return 4000;
	}
}
}

namespace pocketmine\block {
use pocketmine\block\utils\Colored;
use pocketmine\block\utils\ColoredTrait;

class DyedCandle extends Candle implements Colored{
	use ColoredTrait;

	protected function getCandleIfCompatibleType(Block $block) : ?Candle{
		$result = parent::getCandleIfCompatibleType($block);
		//different coloured candles can't be combined in the same block
		return $result instanceof DyedCandle && $result->color === $this->color ? $result : null;
	}
}
}

namespace pocketmine\block {
use pocketmine\block\utils\Colored;
use pocketmine\block\utils\ColoredTrait;

final class DyedShulkerBox extends ShulkerBox implements Colored{
	use ColoredTrait;
}
}

namespace pocketmine\block {
class Element extends Opaque{
	public function __construct(
		BlockIdentifier $idInfo,
		string $name,
		BlockTypeInfo $typeInfo,
		private string $symbol,
		private int $atomicWeight,
		private int $group
	){
		parent::__construct($idInfo, $name, $typeInfo);
	}

	public function getAtomicWeight() : int{
		return $this->atomicWeight;
	}

	public function getGroup() : int{
		return $this->group;
	}

	public function getSymbol() : string{
		return $this->symbol;
	}
}
}

namespace pocketmine\block {
use pocketmine\block\utils\FortuneDropHelper;
use pocketmine\item\Item;
use pocketmine\item\VanillaItems;
use function mt_rand;

class EmeraldOre extends Opaque{

	public function getDropsForCompatibleTool(Item $item) : array{
		return [
			VanillaItems::EMERALD()->setCount(FortuneDropHelper::weighted($item, min: 1, maxBase: 1))
		];
	}

	public function isAffectedBySilkTouch() : bool{
		return true;
	}

	protected function getXpDropAmount() : int{
		return mt_rand(3, 7);
	}
}
}

namespace pocketmine\block {
use pocketmine\block\utils\FacesOppositePlacingPlayerTrait;
use pocketmine\block\utils\HorizontalFacing;
use pocketmine\data\runtime\RuntimeDataDescriber;
use pocketmine\math\AxisAlignedBB;
use pocketmine\math\Facing;

class EndPortalFrame extends Opaque implements HorizontalFacing{
	use FacesOppositePlacingPlayerTrait;

	protected bool $eye = false;

	protected function describeBlockOnlyState(RuntimeDataDescriber $w) : void{
		$w->horizontalFacing($this->facing);
		$w->bool($this->eye);
	}

	public function hasEye() : bool{ return $this->eye; }

	public function setEye(bool $eye) : self{
		$this->eye = $eye;
		return $this;
	}

	public function getLightLevel() : int{
		return 1;
	}

	protected function recalculateCollisionBoxes() : array{
		return [AxisAlignedBB::one()->trim(Facing::UP, 3 / 16)];
	}
}
}

namespace pocketmine\block {
use pocketmine\block\utils\SupportType;
use pocketmine\data\runtime\RuntimeDataDescriber;
use pocketmine\entity\Entity;
use pocketmine\entity\Living;
use pocketmine\event\block\FarmlandHydrationChangeEvent;
use pocketmine\event\entity\EntityTrampleFarmlandEvent;
use pocketmine\item\Item;
use pocketmine\math\AxisAlignedBB;
use pocketmine\math\Facing;
use pocketmine\utils\Utils;
use function intdiv;

class Farmland extends Transparent{
	public const MAX_WETNESS = 7;

	private const WATER_SEARCH_HORIZONTAL_LENGTH = 9;

	private const WATER_SEARCH_VERTICAL_LENGTH = 2;

	private const WATER_POSITION_INDEX_UNKNOWN = -1;
	private const WATER_POSITION_INDICES_TOTAL = (self::WATER_SEARCH_HORIZONTAL_LENGTH ** 2) * 2;

	protected int $wetness = 0; //"moisture" blockstate property in PC

	private int $waterPositionIndex = self::WATER_POSITION_INDEX_UNKNOWN;

	protected function describeBlockOnlyState(RuntimeDataDescriber $w) : void{
		$w->boundedIntAuto(0, self::MAX_WETNESS, $this->wetness);
		$w->boundedIntAuto(-1, self::WATER_POSITION_INDICES_TOTAL - 1, $this->waterPositionIndex);
	}

	public function getWetness() : int{ return $this->wetness; }

	public function setWetness(int $wetness) : self{
		if($wetness < 0 || $wetness > self::MAX_WETNESS){
			throw new \InvalidArgumentException("Wetness must be in range 0 ... " . self::MAX_WETNESS);
		}
		$this->wetness = $wetness;
		return $this;
	}

	public function getWaterPositionIndex() : int{ return $this->waterPositionIndex; }

	public function setWaterPositionIndex(int $waterPositionIndex) : self{
		if($waterPositionIndex < -1 || $waterPositionIndex >= self::WATER_POSITION_INDICES_TOTAL){
			throw new \InvalidArgumentException("Water XZ index must be in range -1 ... " . (self::WATER_POSITION_INDICES_TOTAL - 1));
		}
		$this->waterPositionIndex = $waterPositionIndex;
		return $this;
	}

	protected function recalculateCollisionBoxes() : array{
		return [AxisAlignedBB::one()->trim(Facing::UP, 1 / 16)];
	}

	public function getSupportType(int $facing) : SupportType{
		return match($facing){
			Facing::DOWN => SupportType::FULL,
			default => SupportType::NONE,
		};
	}

	public function onNearbyBlockChange() : void{
		if($this->getSide(Facing::UP)->isSolid()){
			$this->position->getWorld()->setBlock($this->position, VanillaBlocks::DIRT());
		}
	}

	public function ticksRandomly() : bool{
		return true;
	}

	public function onRandomTick() : void{
		$world = $this->position->getWorld();

		//this property may be updated by canHydrate() - track this so we know if we need to set the block again
		$oldWaterPositionIndex = $this->waterPositionIndex;
		$changed = false;

		if(!$this->canHydrate()){
			if($this->wetness > 0){
				$event = new FarmlandHydrationChangeEvent($this, $this->wetness, $this->wetness - 1);
				$event->call();
				if(!$event->isCancelled()){
					$this->wetness = $event->getNewHydration();
					$world->setBlock($this->position, $this, false);
					$changed = true;
				}
			}else{
				$world->setBlock($this->position, VanillaBlocks::DIRT());
				$changed = true;
			}
		}elseif($this->wetness < self::MAX_WETNESS){
			$event = new FarmlandHydrationChangeEvent($this, $this->wetness, self::MAX_WETNESS);
			$event->call();
			if(!$event->isCancelled()){
				$this->wetness = $event->getNewHydration();
				$world->setBlock($this->position, $this, false);
				$changed = true;
			}
		}

		if(!$changed && $oldWaterPositionIndex !== $this->waterPositionIndex){
			//ensure the water square index is saved regardless of whether anything else happened
			$world->setBlock($this->position, $this, false);
		}
	}

	public function onEntityLand(Entity $entity) : ?float{
		if($entity instanceof Living && Utils::getRandomFloat() < $entity->getFallDistance() - 0.5){
			$ev = new EntityTrampleFarmlandEvent($entity, $this);
			$ev->call();
			if(!$ev->isCancelled()){
				$this->position->getWorld()->setBlock($this->position, VanillaBlocks::DIRT());
			}
		}
		return null;
	}

	protected function canHydrate() : bool{
		$world = $this->position->getWorld();

		$startX = $this->position->getFloorX() - (int) (self::WATER_SEARCH_HORIZONTAL_LENGTH / 2);
		$startY = $this->position->getFloorY();
		$startZ = $this->position->getFloorZ() - (int) (self::WATER_SEARCH_HORIZONTAL_LENGTH / 2);

		if($this->waterPositionIndex !== self::WATER_POSITION_INDEX_UNKNOWN){
			$raw = $this->waterPositionIndex;
			$x = $raw % self::WATER_SEARCH_HORIZONTAL_LENGTH;
			$raw = intdiv($raw, self::WATER_SEARCH_HORIZONTAL_LENGTH);
			$z = $raw % self::WATER_SEARCH_HORIZONTAL_LENGTH;
			$raw = intdiv($raw, self::WATER_SEARCH_HORIZONTAL_LENGTH);
			$y = $raw % self::WATER_SEARCH_VERTICAL_LENGTH;

			if($world->getBlockAt($startX + $x, $startY + $y, $startZ + $z) instanceof Water){
				return true;
			}
		}

		//no water found at cached position - search the whole area
		//y will increment after x/z have been exhausted, as usually water will be at the same Y as the farmland
		for($y = 0; $y < self::WATER_SEARCH_VERTICAL_LENGTH; $y++){
			for($x = 0; $x < self::WATER_SEARCH_HORIZONTAL_LENGTH; $x++){
				for($z = 0; $z < self::WATER_SEARCH_HORIZONTAL_LENGTH; $z++){
					if($world->getBlockAt($startX + $x, $startY + $y, $startZ + $z) instanceof Water){
						$this->waterPositionIndex = $x + ($z * self::WATER_SEARCH_HORIZONTAL_LENGTH) + ($y * self::WATER_SEARCH_HORIZONTAL_LENGTH ** 2);
						return true;
					}
				}
			}
		}

		$this->waterPositionIndex = self::WATER_POSITION_INDEX_UNKNOWN;
		return false;
	}

	public function getDropsForCompatibleTool(Item $item) : array{
		return [
			VanillaBlocks::DIRT()->asItem()
		];
	}

	public function getPickedItem(bool $addUserData = false) : Item{
		return VanillaBlocks::DIRT()->asItem();
	}
}
}

namespace pocketmine\block {
class FletchingTable extends Opaque{
	public function getFuelTime() : int{
		return 300;
	}
}
}

namespace pocketmine\block {
use pocketmine\block\utils\SignLikeRotation;
use pocketmine\block\utils\SignLikeRotationTrait;
use pocketmine\item\Item;
use pocketmine\math\Facing;
use pocketmine\math\Vector3;
use pocketmine\player\Player;
use pocketmine\world\BlockTransaction;

final class FloorBanner extends BaseBanner implements SignLikeRotation{
	use SignLikeRotationTrait;

	protected function getOminousVersion() : Block{
		return VanillaBlocks::OMINOUS_BANNER()->setRotation($this->rotation);
	}

	protected function getSupportingFace() : int{
		return Facing::DOWN;
	}

	public function place(BlockTransaction $tx, Item $item, Block $blockReplace, Block $blockClicked, int $face, Vector3 $clickVector, ?Player $player = null) : bool{
		if($face !== Facing::UP){
			return false;
		}

		if($player !== null){
			$this->rotation = self::getRotationFromYaw($player->getLocation()->getYaw());
		}
		return parent::place($tx, $item, $blockReplace, $blockClicked, $face, $clickVector, $player);
	}
}
}

namespace pocketmine\block {
use pocketmine\block\utils\StaticSupportTrait;
use pocketmine\data\runtime\RuntimeDataDescriber;
use pocketmine\item\Item;
use pocketmine\item\VanillaItems;
use pocketmine\math\Axis;
use pocketmine\math\Facing;
use pocketmine\math\Vector3;
use pocketmine\player\Player;
use pocketmine\world\BlockTransaction;
use function atan2;
use function rad2deg;

final class FloorCoralFan extends BaseCoral{
	use StaticSupportTrait;

	private int $axis = Axis::X;

	protected function describeBlockOnlyState(RuntimeDataDescriber $w) : void{
		$w->horizontalAxis($this->axis);
	}

	public function getAxis() : int{ return $this->axis; }

	public function setAxis(int $axis) : self{
		if($axis !== Axis::X && $axis !== Axis::Z){
			throw new \InvalidArgumentException("Axis must be X or Z only");
		}
		$this->axis = $axis;
		return $this;
	}

	public function place(BlockTransaction $tx, Item $item, Block $blockReplace, Block $blockClicked, int $face, Vector3 $clickVector, ?Player $player = null) : bool{
		if($player !== null){
			$playerBlockPos = $player->getPosition()->floor();
			$directionVector = $blockReplace->position->subtractVector($playerBlockPos)->normalize();
			$angle = rad2deg(atan2($directionVector->getZ(), $directionVector->getX()));

			if($angle <= 45 || 315 <= $angle || (135 <= $angle && $angle <= 225)){
				//TODO: This produces Z axis 75% of the time, because any negative angle will produce Z axis.
				//This is a bug in vanilla. https://bugs.mojang.com/browse/MCPE-125311
				$this->axis = Axis::Z;
			}
		}

		$this->dead = !$this->isCoveredWithWater();

		return parent::place($tx, $item, $blockReplace, $blockClicked, $face, $clickVector, $player);
	}

	private function canBeSupportedAt(Block $block) : bool{
		return $block->getAdjacentSupportType(Facing::DOWN)->hasCenterSupport();
	}

	public function asItem() : Item{
		return VanillaItems::CORAL_FAN()->setCoralType($this->coralType)->setDead($this->dead);
	}
}
}

namespace pocketmine\block {
use pocketmine\block\utils\SignLikeRotation;
use pocketmine\block\utils\SignLikeRotationTrait;
use pocketmine\item\Item;
use pocketmine\math\Facing;
use pocketmine\math\Vector3;
use pocketmine\player\Player;
use pocketmine\world\BlockTransaction;

final class FloorSign extends BaseSign implements SignLikeRotation{
	use SignLikeRotationTrait;

	protected function getSupportingFace() : int{
		return Facing::DOWN;
	}

	public function place(BlockTransaction $tx, Item $item, Block $blockReplace, Block $blockClicked, int $face, Vector3 $clickVector, ?Player $player = null) : bool{
		if($face !== Facing::UP){
			return false;
		}

		if($player !== null){
			$this->rotation = self::getRotationFromYaw($player->getLocation()->getYaw());
		}
		return parent::place($tx, $item, $blockReplace, $blockClicked, $face, $clickVector, $player);
	}

	protected function getFacingDegrees() : float{
		return $this->rotation * 22.5;
	}
}
}

namespace pocketmine\block {
use pocketmine\block\utils\FroglightType;
use pocketmine\data\runtime\RuntimeDataDescriber;

final class Froglight extends SimplePillar{

	private FroglightType $froglightType = FroglightType::OCHRE;

	public function describeBlockItemState(RuntimeDataDescriber $w) : void{
		$w->enum($this->froglightType);
	}

	public function getFroglightType() : FroglightType{ return $this->froglightType; }

	public function setFroglightType(FroglightType $froglightType) : self{
		$this->froglightType = $froglightType;
		return $this;
	}

	public function getLightLevel() : int{
		return 15;
	}
}
}

namespace pocketmine\block {
use pocketmine\block\utils\BlockEventHelper;
use pocketmine\item\enchantment\VanillaEnchantments;
use pocketmine\item\Item;
use pocketmine\player\Player;

class Ice extends Transparent{

	public function getLightFilter() : int{
		return 2;
	}

	public function getFrictionFactor() : float{
		return 0.98;
	}

	public function onBreak(Item $item, ?Player $player = null, array &$returnedItems = []) : bool{
		if(($player === null || $player->isSurvival()) && !$item->hasEnchantment(VanillaEnchantments::SILK_TOUCH())){
			$this->position->getWorld()->setBlock($this->position, VanillaBlocks::WATER());
			return true;
		}
		return parent::onBreak($item, $player, $returnedItems);
	}

	public function ticksRandomly() : bool{
		return true;
	}

	public function onRandomTick() : void{
		$world = $this->position->getWorld();
		if($world->getHighestAdjacentBlockLight($this->position->x, $this->position->y, $this->position->z) >= 12){
			BlockEventHelper::melt($this, VanillaBlocks::WATER());
		}
	}

	public function getDropsForCompatibleTool(Item $item) : array{
		return [];
	}

	public function isAffectedBySilkTouch() : bool{
		return true;
	}
}
}

namespace pocketmine\block {
use pocketmine\block\utils\Ageable;
use pocketmine\block\utils\AgeableTrait;
use pocketmine\block\utils\BlockEventHelper;
use function mt_rand;

class FrostedIce extends Ice implements Ageable{
	use AgeableTrait;

	public const MAX_AGE = 3;

	public function onNearbyBlockChange() : void{
		$this->position->getWorld()->scheduleDelayedBlockUpdate($this->position, mt_rand(20, 40));
	}

	public function onRandomTick() : void{
		$world = $this->position->getWorld();
		if((!$this->checkAdjacentBlocks(4) || mt_rand(0, 2) === 0) &&
			$world->getHighestAdjacentFullLightAt($this->position->x, $this->position->y, $this->position->z) >= 12 - $this->age){
			if($this->tryMelt()){
				foreach($this->getAllSides() as $block){
					if($block instanceof FrostedIce){
						$block->tryMelt();
					}
				}
			}
		}else{
			$world->scheduleDelayedBlockUpdate($this->position, mt_rand(20, 40));
		}
	}

	public function onScheduledUpdate() : void{
		$this->onRandomTick();
	}

	private function checkAdjacentBlocks(int $requirement) : bool{
		$found = 0;
		for($x = -1; $x <= 1; ++$x){
			for($z = -1; $z <= 1; ++$z){
				if($x === 0 && $z === 0){
					continue;
				}
				if(
					$this->position->getWorld()->getBlockAt($this->position->x + $x, $this->position->y, $this->position->z + $z) instanceof FrostedIce &&
					++$found >= $requirement
				){
					return true;
				}
			}
		}
		return false;
	}

	private function tryMelt() : bool{
		$world = $this->position->getWorld();
		if($this->age >= self::MAX_AGE){
			BlockEventHelper::melt($this, VanillaBlocks::WATER());
			return true;
		}

		$this->age++;
		$world->setBlock($this->position, $this);
		$world->scheduleDelayedBlockUpdate($this->position, mt_rand(20, 40));
		return false;
	}

	public function isAffectedBySilkTouch() : bool{
		return false;
	}
}
}

namespace pocketmine\block {
use pocketmine\block\utils\FortuneDropHelper;
use pocketmine\item\Item;
use pocketmine\item\VanillaItems;
use function mt_rand;

final class GildedBlackstone extends Opaque{

	public function getDropsForCompatibleTool(Item $item) : array{
		if(FortuneDropHelper::bonusChanceDivisor($item, 10, 3)){
			return [VanillaItems::GOLD_NUGGET()->setCount(mt_rand(2, 5))];
		}

		return parent::getDropsForCompatibleTool($item);
	}

	public function isAffectedBySilkTouch() : bool{ return true; }
}
}

namespace pocketmine\block {
use pocketmine\block\utils\Colored;
use pocketmine\block\utils\ColoredTrait;
use pocketmine\block\utils\FacesOppositePlacingPlayerTrait;
use pocketmine\block\utils\HorizontalFacing;

class GlazedTerracotta extends Opaque implements Colored, HorizontalFacing{
	use ColoredTrait;
	use FacesOppositePlacingPlayerTrait;
}
}

namespace pocketmine\block {
class GlowingObsidian extends Opaque{

	public function getLightLevel() : int{
		return 12;
	}
}
}

namespace pocketmine\block {
use pocketmine\block\utils\FortuneDropHelper;
use pocketmine\item\Item;
use pocketmine\item\VanillaItems;
use function min;

class Glowstone extends Transparent{

	public function getLightLevel() : int{
		return 15;
	}

	public function getDropsForCompatibleTool(Item $item) : array{
		return [
			VanillaItems::GLOWSTONE_DUST()->setCount(min(4, FortuneDropHelper::discrete($item, 2, 4)))
		];
	}

	public function isAffectedBySilkTouch() : bool{
		return true;
	}
}
}

namespace pocketmine\block {
use pocketmine\block\utils\FortuneDropHelper;
use pocketmine\item\Item;
use pocketmine\item\VanillaItems;

final class GoldOre extends Opaque{

	public function getDropsForCompatibleTool(Item $item) : array{
		return [VanillaItems::RAW_GOLD()->setCount(FortuneDropHelper::weighted($item, min: 1, maxBase: 1))];
	}

	public function isAffectedBySilkTouch() : bool{ return true; }
}
}

namespace pocketmine\block {
use pocketmine\block\utils\BlockEventHelper;
use pocketmine\block\utils\DirtType;
use pocketmine\item\Fertilizer;
use pocketmine\item\Hoe;
use pocketmine\item\Item;
use pocketmine\item\Shovel;
use pocketmine\math\Facing;
use pocketmine\math\Vector3;
use pocketmine\player\Player;
use pocketmine\utils\Random;
use pocketmine\world\generator\object\TallGrass as TallGrassObject;
use pocketmine\world\sound\ItemUseOnBlockSound;
use function mt_rand;

class Grass extends Opaque{

	public function getDropsForCompatibleTool(Item $item) : array{
		return [
			VanillaBlocks::DIRT()->asItem()
		];
	}

	public function isAffectedBySilkTouch() : bool{
		return true;
	}

	public function ticksRandomly() : bool{
		return true;
	}

	public function onRandomTick() : void{
		$world = $this->position->getWorld();
		$lightAbove = $world->getFullLightAt($this->position->x, $this->position->y + 1, $this->position->z);
		if($lightAbove < 4 && $world->getBlockAt($this->position->x, $this->position->y + 1, $this->position->z)->getLightFilter() >= 2){
			//grass dies
			BlockEventHelper::spread($this, VanillaBlocks::DIRT(), $this);
		}elseif($lightAbove >= 9){
			//try grass spread
			for($i = 0; $i < 4; ++$i){
				$x = mt_rand($this->position->x - 1, $this->position->x + 1);
				$y = mt_rand($this->position->y - 3, $this->position->y + 1);
				$z = mt_rand($this->position->z - 1, $this->position->z + 1);

				$b = $world->getBlockAt($x, $y, $z);
				if(
					!($b instanceof Dirt) ||
					$b->getDirtType() !== DirtType::NORMAL ||
					$world->getFullLightAt($x, $y + 1, $z) < 4 ||
					$world->getBlockAt($x, $y + 1, $z)->getLightFilter() >= 2
				){
					continue;
				}

				BlockEventHelper::spread($b, VanillaBlocks::GRASS(), $this);
			}
		}
	}

	public function onInteract(Item $item, int $face, Vector3 $clickVector, ?Player $player = null, array &$returnedItems = []) : bool{
		if($this->getSide(Facing::UP)->getTypeId() !== BlockTypeIds::AIR){
			return false;
		}
		$world = $this->position->getWorld();
		if($item instanceof Fertilizer){
			$item->pop();
			TallGrassObject::growGrass($world, $this->position, new Random(mt_rand()), 8, 2);

			return true;
		}
		if($face !== Facing::DOWN){
			if($item instanceof Hoe){
				$item->applyDamage(1);
				$newBlock = VanillaBlocks::FARMLAND();
				$world->addSound($this->position->add(0.5, 0.5, 0.5), new ItemUseOnBlockSound($newBlock));
				$world->setBlock($this->position, $newBlock);

				return true;
			}elseif($item instanceof Shovel){
				$item->applyDamage(1);
				$newBlock = VanillaBlocks::GRASS_PATH();
				$world->addSound($this->position->add(0.5, 0.5, 0.5), new ItemUseOnBlockSound($newBlock));
				$world->setBlock($this->position, $newBlock);

				return true;
			}
		}

		return false;
	}
}
}

namespace pocketmine\block {
use pocketmine\block\utils\SupportType;
use pocketmine\item\Item;
use pocketmine\math\AxisAlignedBB;
use pocketmine\math\Facing;

class GrassPath extends Transparent{

	protected function recalculateCollisionBoxes() : array{
		return [AxisAlignedBB::one()->trim(Facing::UP, 1 / 16)];
	}

	public function getSupportType(int $facing) : SupportType{
		return match($facing){
			Facing::UP => SupportType::EDGE,
			Facing::DOWN => SupportType::FULL,
			default => SupportType::NONE,
		};
	}

	public function onNearbyBlockChange() : void{
		if($this->getSide(Facing::UP)->isSolid()){
			$this->position->getWorld()->setBlock($this->position, VanillaBlocks::DIRT());
		}
	}

	public function getDropsForCompatibleTool(Item $item) : array{
		return [
			VanillaBlocks::DIRT()->asItem()
		];
	}

	public function isAffectedBySilkTouch() : bool{
		return true;
	}
}
}

namespace pocketmine\block {
use pocketmine\block\utils\Fallable;
use pocketmine\block\utils\FallableTrait;
use pocketmine\block\utils\FortuneDropHelper;
use pocketmine\item\Item;
use pocketmine\item\VanillaItems;

class Gravel extends Opaque implements Fallable{
	use FallableTrait;

	public function getDropsForCompatibleTool(Item $item) : array{
		if(FortuneDropHelper::bonusChanceDivisor($item, 10, 3)){
			return [
				VanillaItems::FLINT()
			];
		}

		return parent::getDropsForCompatibleTool($item);
	}

	public function isAffectedBySilkTouch() : bool{
		return true;
	}
}
}

namespace pocketmine\block {
class HardenedClay extends Opaque{

}
}

namespace pocketmine\block {
use pocketmine\block\utils\PillarRotation;
use pocketmine\block\utils\PillarRotationTrait;
use pocketmine\entity\Entity;

class HayBale extends Opaque implements PillarRotation{
	use PillarRotationTrait;

	public function getFlameEncouragement() : int{
		return 60;
	}

	public function getFlammability() : int{
		return 20;
	}

	public function onEntityLand(Entity $entity) : ?float{
		$entity->fallDistance *= 0.2;
		return null;
	}
}
}

namespace pocketmine\block {
use pocketmine\item\Item;

class InfestedStone extends Opaque{

	private int $imitated;

	public function __construct(BlockIdentifier $idInfo, string $name, BlockTypeInfo $typeInfo, Block $imitated){
		parent::__construct($idInfo, $name, $typeInfo);
		$this->imitated = $imitated->getStateId();
	}

	public function getImitatedBlock() : Block{
		return RuntimeBlockStateRegistry::getInstance()->fromStateId($this->imitated);
	}

	public function getDropsForCompatibleTool(Item $item) : array{
		return [];
	}

	public function getSilkTouchDrops(Item $item) : array{
		return [$this->getImitatedBlock()->asItem()];
	}

	public function isAffectedBySilkTouch() : bool{
		return true;
	}

	//TODO
}
}

namespace pocketmine\block {
use pocketmine\block\utils\PillarRotation;
use pocketmine\block\utils\PillarRotationTrait;

class InfestedPillar extends InfestedStone implements PillarRotation{
	use PillarRotationTrait;
}
}

namespace pocketmine\block {
use pocketmine\block\utils\FortuneDropHelper;
use pocketmine\item\Item;
use pocketmine\item\VanillaItems;

final class IronOre extends Opaque{

	public function getDropsForCompatibleTool(Item $item) : array{
		return [VanillaItems::RAW_IRON()->setCount(FortuneDropHelper::weighted($item, min: 1, maxBase: 1))];
	}

	public function isAffectedBySilkTouch() : bool{ return true; }
}
}

namespace pocketmine\block {
use pocketmine\block\utils\FortuneDropHelper;
use pocketmine\item\Item;
use pocketmine\item\VanillaItems;
use function mt_rand;

class LapisOre extends Opaque{

	public function getDropsForCompatibleTool(Item $item) : array{
		return [
			VanillaItems::LAPIS_LAZULI()->setCount(FortuneDropHelper::weighted($item, min: 4, maxBase: 9))
		];
	}

	public function isAffectedBySilkTouch() : bool{
		return true;
	}

	protected function getXpDropAmount() : int{
		return mt_rand(2, 5);
	}
}
}

namespace pocketmine\block {
use pocketmine\data\runtime\RuntimeDataDescriber;
use pocketmine\item\Item;
use pocketmine\math\Vector3;
use pocketmine\player\Player;

final class Light extends Flowable{
	public const MIN_LIGHT_LEVEL = 0;
	public const MAX_LIGHT_LEVEL = 15;

	private int $level = self::MAX_LIGHT_LEVEL;

	public function describeBlockItemState(RuntimeDataDescriber $w) : void{
		$w->boundedIntAuto(self::MIN_LIGHT_LEVEL, self::MAX_LIGHT_LEVEL, $this->level);
	}

	public function getLightLevel() : int{ return $this->level; }

	public function setLightLevel(int $level) : self{
		if($level < self::MIN_LIGHT_LEVEL || $level > self::MAX_LIGHT_LEVEL){
			throw new \InvalidArgumentException("Light level must be in the range " . self::MIN_LIGHT_LEVEL . " ... " . self::MAX_LIGHT_LEVEL);
		}
		$this->level = $level;
		return $this;
	}

	public function canBeReplaced() : bool{ return true; }

	public function canBePlacedAt(Block $blockReplace, Vector3 $clickVector, int $face, bool $isClickedBlock) : bool{
		//light blocks behave like solid blocks when placing them on another light block
		return $blockReplace->canBeReplaced() && $blockReplace->getTypeId() !== $this->getTypeId();
	}

	public function onInteract(Item $item, int $face, Vector3 $clickVector, ?Player $player = null, array &$returnedItems = []) : bool{
		$this->level = $this->level === self::MAX_LIGHT_LEVEL ?
			self::MIN_LIGHT_LEVEL :
			$this->level + 1;

		$this->position->getWorld()->setBlock($this->position, $this);

		return true;
	}
}
}

namespace pocketmine\block {
class LitPumpkin extends CarvedPumpkin{

	public function getLightLevel() : int{
		return 15;
	}
}
}

namespace pocketmine\block {
use pocketmine\block\inventory\LoomInventory;
use pocketmine\block\utils\FacesOppositePlacingPlayerTrait;
use pocketmine\block\utils\HorizontalFacing;
use pocketmine\item\Item;
use pocketmine\math\Vector3;
use pocketmine\player\Player;

final class Loom extends Opaque implements HorizontalFacing{
	use FacesOppositePlacingPlayerTrait;

	public function onInteract(Item $item, int $face, Vector3 $clickVector, ?Player $player = null, array &$returnedItems = []) : bool{
		if($player !== null){
			$player->setCurrentWindow(new LoomInventory($this->position));
			return true;
		}
		return false;
	}
}
}

namespace pocketmine\block {
use pocketmine\block\utils\FortuneDropHelper;
use pocketmine\item\Item;
use pocketmine\item\VanillaItems;
use function min;

class Melon extends Opaque{

	public function getDropsForCompatibleTool(Item $item) : array{
		return [
			VanillaItems::MELON()->setCount(min(9, FortuneDropHelper::discrete($item, 3, 7)))
		];
	}

	public function isAffectedBySilkTouch() : bool{
		return true;
	}
}
}

namespace pocketmine\block {
use pocketmine\block\utils\BlockEventHelper;
use pocketmine\block\utils\DirtType;
use pocketmine\item\Item;
use pocketmine\math\Facing;
use function mt_rand;

class Mycelium extends Opaque{

	public function getDropsForCompatibleTool(Item $item) : array{
		return [
			VanillaBlocks::DIRT()->asItem()
		];
	}

	public function isAffectedBySilkTouch() : bool{
		return true;
	}

	public function ticksRandomly() : bool{
		return true;
	}

	public function onRandomTick() : void{
		//TODO: light levels
		$x = mt_rand($this->position->x - 1, $this->position->x + 1);
		$y = mt_rand($this->position->y - 2, $this->position->y + 2);
		$z = mt_rand($this->position->z - 1, $this->position->z + 1);
		$world = $this->position->getWorld();
		$block = $world->getBlockAt($x, $y, $z);
		if($block instanceof Dirt && $block->getDirtType() === DirtType::NORMAL){
			if($block->getSide(Facing::UP) instanceof Transparent){
				BlockEventHelper::spread($block, VanillaBlocks::MYCELIUM(), $this);
			}
		}
	}
}
}

namespace pocketmine\block {
use pocketmine\block\utils\FortuneDropHelper;
use pocketmine\item\Item;
use pocketmine\item\VanillaItems;

final class NetherGoldOre extends Opaque{

	public function getDropsForCompatibleTool(Item $item) : array{
		return [VanillaItems::GOLD_NUGGET()->setCount(FortuneDropHelper::weighted($item, min: 2, maxBase: 6))];
	}

	public function isAffectedBySilkTouch() : bool{ return true; }
}
}

namespace pocketmine\block {
use pocketmine\block\utils\SupportType;
use pocketmine\data\runtime\RuntimeDataDescriber;
use pocketmine\entity\Entity;
use pocketmine\item\Item;
use pocketmine\math\Axis;

class NetherPortal extends Transparent{

	protected int $axis = Axis::X;

	protected function describeBlockOnlyState(RuntimeDataDescriber $w) : void{
		$w->horizontalAxis($this->axis);
	}

	public function getAxis() : int{
		return $this->axis;
	}

	public function setAxis(int $axis) : self{
		if($axis !== Axis::X && $axis !== Axis::Z){
			throw new \InvalidArgumentException("Invalid axis");
		}
		$this->axis = $axis;
		return $this;
	}

	public function getLightLevel() : int{
		return 11;
	}

	public function isSolid() : bool{
		return false;
	}

	protected function recalculateCollisionBoxes() : array{
		return [];
	}

	public function getSupportType(int $facing) : SupportType{
		return SupportType::NONE;
	}

	public function getDrops(mixed $item = null) : array{
		return [];
	}

	public function onEntityInside(Entity $entity) : bool{
		//TODO
		return true;
	}
}
}

namespace pocketmine\block {
use pocketmine\block\utils\FortuneDropHelper;
use pocketmine\item\Item;
use pocketmine\item\VanillaItems;
use function mt_rand;

class NetherQuartzOre extends Opaque{

	public function getDropsForCompatibleTool(Item $item) : array{
		return [
			VanillaItems::NETHER_QUARTZ()->setCount(FortuneDropHelper::weighted($item, min: 1, maxBase: 1))
		];
	}

	public function isAffectedBySilkTouch() : bool{
		return true;
	}

	protected function getXpDropAmount() : int{
		return mt_rand(2, 5);
	}
}
}

namespace pocketmine\block {
use pocketmine\item\Item;
use pocketmine\item\VanillaItems;

class NetherReactor extends Opaque{

	public function getDropsForCompatibleTool(Item $item) : array{
		return [
			VanillaItems::IRON_INGOT()->setCount(6),
			VanillaItems::DIAMOND()->setCount(3)
		];
	}
}
}

namespace pocketmine\block {
use pocketmine\item\Fertilizer;
use pocketmine\item\Item;
use pocketmine\math\Vector3;
use pocketmine\player\Player;
use function mt_rand;

class Netherrack extends Opaque{

	public function burnsForever() : bool{
		return true;
	}

	public function onInteract(Item $item, int $face, Vector3 $clickVector, ?Player $player = null, array &$returnedItems = []) : bool{
		if($item instanceof Fertilizer){
			if($this->tryTransform()){
				$item->pop();
				return true;
			}
		}
		return false;
	}

	private function tryTransform() : bool{
		$world = $this->position->getWorld();
		$pos = $this->position;

		if(!$world->getBlock($pos->up())->isTransparent()){
			return false;
		}

		$hasWarpedNylium = false;
		$hasCrimsonNylium = false;

		for($x = -1; $x <= 1; $x++){
			for($y = -1; $y <= 1; $y++){
				for($z = -1; $z <= 1; $z++){
					$blockTypeId = $world->getBlock($pos->add($x, $y, $z))->getTypeId();

					if($blockTypeId === BlockTypeIds::WARPED_NYLIUM){
						$hasWarpedNylium = true;
					}elseif($blockTypeId === BlockTypeIds::CRIMSON_NYLIUM){
						$hasCrimsonNylium = true;
					}

					if($hasWarpedNylium && $hasCrimsonNylium){
						break 3;
					}
				}
			}
		}

		if(!$hasWarpedNylium && !$hasCrimsonNylium){
			return false;
		}

		$world->setBlock($pos, match(true){
			$hasWarpedNylium && $hasCrimsonNylium => (mt_rand(0, 1) === 0 ? VanillaBlocks::WARPED_NYLIUM() : VanillaBlocks::CRIMSON_NYLIUM()),
			$hasWarpedNylium => VanillaBlocks::WARPED_NYLIUM(),
			$hasCrimsonNylium => VanillaBlocks::CRIMSON_NYLIUM(),
		});
		return true;
	}
}
}

namespace pocketmine\block {
use pocketmine\block\utils\BlockEventHelper;
use pocketmine\item\Fertilizer;
use pocketmine\item\Item;
use pocketmine\math\Facing;
use pocketmine\math\Vector3;
use pocketmine\player\Player;
use pocketmine\utils\Random;
use function count;

class Nylium extends Opaque{

	public function __construct(BlockIdentifier $idInfo, string $name, BlockTypeInfo $typeInfo, private readonly array $vegetation){
		parent::__construct($idInfo, $name, $typeInfo);
	}

	public function getDropsForCompatibleTool(Item $item) : array{
		return [
			VanillaBlocks::NETHERRACK()->asItem()
		];
	}

	public function isAffectedBySilkTouch() : bool{
		return true;
	}

	public function ticksRandomly() : bool{
		return true;
	}

	public function onRandomTick() : void{
		$world = $this->position->getWorld();
		$above = $this->getSide(Facing::UP);
		if(!$above->isTransparent()){
			BlockEventHelper::spread($this, VanillaBlocks::NETHERRACK(), $this);
			return;
		}

		$random = new Random();
		for($i = 0; $i < 4; ++$i){
			$pos = $this->position->add($random->nextRange(-1, 1), $random->nextRange(-2, 0), $random->nextRange(-1, 1));
			$block = $world->getBlock($pos);
			if($block->getTypeId() === BlockTypeIds::NETHERRACK && $world->getBlock($pos->up())->isTransparent()){
				BlockEventHelper::spread($block, $this, $this);
			}
		}
	}

	public function onInteract(Item $item, int $face, Vector3 $clickVector, ?Player $player = null, array &$returnedItems = []) : bool{
		if($item instanceof Fertilizer && $this->getSide(Facing::UP)->getTypeId() === BlockTypeIds::AIR){
			$item->pop();
			$this->growVegetation(new Random());
			return true;
		}
		return false;
	}

	private function growVegetation(Random $random) : void{
		$world = $this->position->getWorld();

		for($x = -2; $x <= 2; ++$x){
			for($z = -2; $z <= 2; ++$z){
				if($random->nextBoundedInt(3) === 0){
					$pos = $this->position->add($x, 1, $z);
					$replace = $world->getBlock($pos);
					$place = $this->vegetation[$random->nextBoundedInt(count($this->vegetation))];
					if($world->isInWorld($pos->x, $pos->y, $pos->z) && $replace->getTypeId() === BlockTypeIds::AIR && $place->canBePlacedAt($replace, Vector3::zero(), Facing::DOWN, true)){
						$world->setBlock($pos, $place);
					}
				}
			}
		}
	}
}
}

namespace pocketmine\block {
use pocketmine\block\utils\SignLikeRotation;
use pocketmine\block\utils\SignLikeRotationTrait;
use pocketmine\item\Item;
use pocketmine\math\Facing;
use pocketmine\math\Vector3;
use pocketmine\player\Player;
use pocketmine\world\BlockTransaction;

final class OminousFloorBanner extends BaseOminousBanner implements SignLikeRotation{
	use SignLikeRotationTrait;

	//TODO: duplicated code :(

	protected function getSupportingFace() : int{
		return Facing::DOWN;
	}

	public function place(BlockTransaction $tx, Item $item, Block $blockReplace, Block $blockClicked, int $face, Vector3 $clickVector, ?Player $player = null) : bool{
		if($face !== Facing::UP){
			return false;
		}

		if($player !== null){
			$this->rotation = self::getRotationFromYaw($player->getLocation()->getYaw());
		}
		return parent::place($tx, $item, $blockReplace, $blockClicked, $face, $clickVector, $player);
	}
}
}

namespace pocketmine\block {
use pocketmine\block\utils\HorizontalFacing;
use pocketmine\block\utils\HorizontalFacingTrait;
use pocketmine\item\Item;
use pocketmine\math\Axis;
use pocketmine\math\Facing;
use pocketmine\math\Vector3;
use pocketmine\player\Player;
use pocketmine\world\BlockTransaction;

final class OminousWallBanner extends BaseOminousBanner implements HorizontalFacing{
	use HorizontalFacingTrait;

	protected function getSupportingFace() : int{
		return Facing::opposite($this->facing);
	}

	public function place(BlockTransaction $tx, Item $item, Block $blockReplace, Block $blockClicked, int $face, Vector3 $clickVector, ?Player $player = null) : bool{
		if(Facing::axis($face) === Axis::Y){
			return false;
		}
		$this->facing = $face;
		return parent::place($tx, $item, $blockReplace, $blockClicked, $face, $clickVector, $player);
	}
}
}

namespace pocketmine\block {
use pocketmine\item\Item;

class PackedIce extends Opaque{

	public function getFrictionFactor() : float{
		return 0.98;
	}

	public function getDropsForCompatibleTool(Item $item) : array{
		return [];
	}

	public function isAffectedBySilkTouch() : bool{
		return true;
	}
}
}

namespace pocketmine\block {
use pocketmine\item\Item;

class Podzol extends Opaque{

	public function isAffectedBySilkTouch() : bool{
		return true;
	}

	public function getDropsForCompatibleTool(Item $item) : array{
		return [
			VanillaBlocks::DIRT()->asItem()
		];
	}
}
}

namespace pocketmine\block {
use pocketmine\item\Item;
use pocketmine\item\Shears;
use pocketmine\item\VanillaItems;
use pocketmine\math\Facing;
use pocketmine\math\Vector3;
use pocketmine\player\Player;
use function in_array;

class Pumpkin extends Opaque{

	public function onInteract(Item $item, int $face, Vector3 $clickVector, ?Player $player = null, array &$returnedItems = []) : bool{
		if($item instanceof Shears && in_array($face, Facing::HORIZONTAL, true)){
			$item->applyDamage(1);
			$world = $this->position->getWorld();
			$world->setBlock($this->position, VanillaBlocks::CARVED_PUMPKIN()->setFacing($face));
			$world->dropItem($this->position->add(0.5, 0.5, 0.5), VanillaItems::PUMPKIN_SEEDS()->setCount(1));
			return true;
		}
		return false;
	}
}
}

namespace pocketmine\block {
use pocketmine\block\utils\RedstoneSignalEmitter;

class Redstone extends Opaque implements RedstoneSignalEmitter{

}
}

namespace pocketmine\block {
use pocketmine\block\utils\Lightable;
use pocketmine\block\utils\PoweredByRedstone;
use pocketmine\block\utils\PoweredByRedstoneTrait;
use pocketmine\data\runtime\RuntimeDataDescriber;

class RedstoneLamp extends Opaque implements PoweredByRedstone, Lightable{
	use PoweredByRedstoneTrait;

	protected function describeBlockOnlyState(RuntimeDataDescriber $w) : void{
		$w->bool($this->powered);
	}

	public function getLightLevel() : int{
		return $this->powered ? 15 : 0;
	}

	public function isLit() : bool{
		return $this->powered;
	}

	public function setLit(bool $lit = true) : self{
		$this->powered = $lit;
		return $this;
	}
}
}

namespace pocketmine\block {
use pocketmine\block\utils\FortuneDropHelper;
use pocketmine\block\utils\Lightable;
use pocketmine\block\utils\LightableTrait;
use pocketmine\item\Item;
use pocketmine\item\VanillaItems;
use pocketmine\math\Vector3;
use pocketmine\player\Player;
use function mt_rand;

class RedstoneOre extends Opaque implements Lightable{
	use LightableTrait;

	public function getLightLevel() : int{
		return $this->lit ? 9 : 0;
	}

	public function onInteract(Item $item, int $face, Vector3 $clickVector, ?Player $player = null, array &$returnedItems = []) : bool{
		if(!$this->lit){
			$this->lit = true;
			$this->position->getWorld()->setBlock($this->position, $this); //no return here - this shouldn't prevent block placement
		}
		return false;
	}

	public function onNearbyBlockChange() : void{
		if(!$this->lit){
			$this->lit = true;
			$this->position->getWorld()->setBlock($this->position, $this);
		}
	}

	public function ticksRandomly() : bool{
		return $this->lit;
	}

	public function onRandomTick() : void{
		if($this->lit){
			$this->lit = false;
			$this->position->getWorld()->setBlock($this->position, $this);
		}
	}

	public function getDropsForCompatibleTool(Item $item) : array{
		return [
			VanillaItems::REDSTONE_DUST()->setCount(FortuneDropHelper::discrete($item, 4, 5))
		];
	}

	public function isAffectedBySilkTouch() : bool{
		return true;
	}

	protected function getXpDropAmount() : int{
		return mt_rand(1, 5);
	}
}
}

namespace pocketmine\block {
use pocketmine\block\utils\SupportType;
use pocketmine\data\runtime\RuntimeDataDescriber;
use pocketmine\item\Item;
use pocketmine\math\Facing;
use pocketmine\math\Vector3;
use pocketmine\player\Player;
use pocketmine\world\BlockTransaction;

class Torch extends Flowable{

	protected int $facing = Facing::UP;

	protected function describeBlockOnlyState(RuntimeDataDescriber $w) : void{
		$w->facingExcept($this->facing, Facing::DOWN);
	}

	public function getFacing() : int{ return $this->facing; }

	public function setFacing(int $facing) : self{
		if($facing === Facing::DOWN){
			throw new \InvalidArgumentException("Torch may not face DOWN");
		}
		$this->facing = $facing;
		return $this;
	}

	public function getLightLevel() : int{
		return 14;
	}

	public function onNearbyBlockChange() : void{
		if(!$this->canBeSupportedAt($this, Facing::opposite($this->facing))){
			$this->position->getWorld()->useBreakOn($this->position);
		}
	}

	public function place(BlockTransaction $tx, Item $item, Block $blockReplace, Block $blockClicked, int $face, Vector3 $clickVector, ?Player $player = null) : bool{
		if($face !== Facing::DOWN && $this->canBeSupportedAt($blockReplace, Facing::opposite($face))){
			$this->facing = $face;
			return parent::place($tx, $item, $blockReplace, $blockClicked, $face, $clickVector, $player);
		}else{
			foreach([
				Facing::SOUTH,
				Facing::WEST,
				Facing::NORTH,
				Facing::EAST,
				Facing::DOWN
			] as $side){
				if($this->canBeSupportedAt($blockReplace, $side)){
					$this->facing = Facing::opposite($side);
					return parent::place($tx, $item, $blockReplace, $blockClicked, $face, $clickVector, $player);
				}
			}
		}
		return false;
	}

	private function canBeSupportedAt(Block $block, int $face) : bool{
		return $face === Facing::DOWN ?
			$block->getAdjacentSupportType($face)->hasCenterSupport() :
			$block->getAdjacentSupportType($face) === SupportType::FULL;
	}
}
}

namespace pocketmine\block {
use pocketmine\block\utils\Lightable;
use pocketmine\block\utils\LightableTrait;
use pocketmine\block\utils\RedstoneSignalEmitter;
use pocketmine\data\runtime\RuntimeDataDescriber;

class RedstoneTorch extends Torch implements Lightable, RedstoneSignalEmitter{
	use LightableTrait;

	public function __construct(BlockIdentifier $idInfo, string $name, BlockTypeInfo $typeInfo){
		$this->lit = true;
		parent::__construct($idInfo, $name, $typeInfo);
	}

	protected function describeBlockOnlyState(RuntimeDataDescriber $w) : void{
		parent::describeBlockOnlyState($w);
		$w->bool($this->lit);
	}

	public function getLightLevel() : int{
		return $this->lit ? 7 : 0;
	}
}
}

namespace pocketmine\block {
class Reserved6 extends Opaque{

}
}

namespace pocketmine\block {
use pocketmine\block\utils\MultiAnyFacing;
use pocketmine\block\utils\MultiAnySupportTrait;
use pocketmine\block\utils\SupportType;

final class ResinClump extends Transparent implements MultiAnyFacing{
	use MultiAnySupportTrait;

	public function isSolid() : bool{
		return false;
	}

	public function getSupportType(int $facing) : SupportType{
		return SupportType::NONE;
	}

	public function canBeReplaced() : bool{
		return true;
	}

	protected function getInitialPlaceFaces(Block $blockReplace) : array{
		return $blockReplace instanceof ResinClump ? $blockReplace->faces : [];
	}

	protected function recalculateCollisionBoxes() : array{
		return [];
	}
}
}

namespace pocketmine\block {
use pocketmine\data\runtime\RuntimeDataDescriber;
use pocketmine\event\block\BlockPreExplodeEvent;
use pocketmine\event\player\PlayerRespawnAnchorUseEvent;
use pocketmine\item\Item;
use pocketmine\item\ItemTypeIds;
use pocketmine\lang\KnownTranslationFactory;
use pocketmine\math\Vector3;
use pocketmine\player\Player;
use pocketmine\utils\TextFormat;
use pocketmine\world\Explosion;
use pocketmine\world\Position;
use pocketmine\world\sound\RespawnAnchorChargeSound;
use pocketmine\world\sound\RespawnAnchorSetSpawnSound;

final class RespawnAnchor extends Opaque{
	private const MIN_CHARGES = 0;
	private const MAX_CHARGES = 4;

	private int $charges = self::MIN_CHARGES;

	protected function describeBlockOnlyState(RuntimeDataDescriber $w) : void{
		$w->boundedIntAuto(self::MIN_CHARGES, self::MAX_CHARGES, $this->charges);
	}

	public function getCharges() : int{
		return $this->charges;
	}

	public function setCharges(int $charges) : self{
		if($charges < self::MIN_CHARGES || $charges > self::MAX_CHARGES){
			throw new \InvalidArgumentException("Charges must be between " . self::MIN_CHARGES . " and " . self::MAX_CHARGES . ", given: $charges");
		}
		$this->charges = $charges;
		return $this;
	}

	public function getLightLevel() : int{
		return $this->charges > 0 ? ($this->charges * 4) - 1 : 0;
	}

	public function onInteract(Item $item, int $face, Vector3 $clickVector, ?Player $player = null, array &$returnedItems = []) : bool{
		if($item->getTypeId() === ItemTypeIds::fromBlockTypeId(BlockTypeIds::GLOWSTONE) && $this->charges < self::MAX_CHARGES){
			$this->position->getWorld()->setBlock($this->position, $this->setCharges($this->charges + 1));
			$this->position->getWorld()->addSound($this->position, new RespawnAnchorChargeSound());
			$item->pop();
			return true;
		}

		if($this->charges > self::MIN_CHARGES){
			if($player === null){
				return false;
			}

			$ev = new PlayerRespawnAnchorUseEvent($player, $this, PlayerRespawnAnchorUseEvent::ACTION_EXPLODE);
			$ev->call();
			if($ev->isCancelled()){
				return false;
			}

			switch($ev->getAction()){
				case PlayerRespawnAnchorUseEvent::ACTION_EXPLODE:
					$this->explode($player);
					return true;

				case PlayerRespawnAnchorUseEvent::ACTION_SET_SPAWN:
					if($player->getSpawn() !== null && $player->getSpawn()->equals($this->position)){
						return true;
					}

					$player->setSpawn($this->position);
					$this->position->getWorld()->addSound($this->position, new RespawnAnchorSetSpawnSound());
					$player->sendMessage(KnownTranslationFactory::tile_respawn_anchor_respawnSet()->prefix(TextFormat::GRAY));
					return true;
			}
		}
		return false;
	}

	private function explode(?Player $player) : void{
		$ev = new BlockPreExplodeEvent($this, 5, $player);
		$ev->setIncendiary(true);

		$ev->call();
		if($ev->isCancelled()){
			return;
		}

		$this->position->getWorld()->setBlock($this->position, VanillaBlocks::AIR());

		$explosion = new Explosion(Position::fromObject($this->position->add(0.5, 0.5, 0.5), $this->position->getWorld()), $ev->getRadius(), $this);
		$explosion->setFireChance($ev->getFireChance());

		if($ev->isBlockBreaking()){
			$explosion->explodeA();
		}
		$explosion->explodeB();
	}
}
}

namespace pocketmine\block {
use pocketmine\block\utils\Fallable;
use pocketmine\block\utils\FallableTrait;

class Sand extends Opaque implements Fallable{
	use FallableTrait;
}
}

namespace pocketmine\block {
use pocketmine\item\Item;

final class Sculk extends Opaque{

	public function getDropsForCompatibleTool(Item $item) : array{
		return [];
	}

	public function isAffectedBySilkTouch() : bool{
		return true;
	}

	public function getXpDropAmount() : int{
		return 1;
	}
}
}

namespace pocketmine\block {
use pocketmine\block\utils\FortuneDropHelper;
use pocketmine\item\Item;
use pocketmine\item\VanillaItems;
use function min;

class SeaLantern extends Transparent{

	public function getLightLevel() : int{
		return 15;
	}

	public function getDropsForCompatibleTool(Item $item) : array{
		return [
			VanillaItems::PRISMARINE_CRYSTALS()->setCount(min(5, FortuneDropHelper::discrete($item, 2, 3)))
		];
	}

	public function isAffectedBySilkTouch() : bool{
		return true;
	}
}
}

namespace pocketmine\block {
use pocketmine\entity\Entity;
use pocketmine\entity\Living;

final class Slime extends Transparent{

	public function getFrictionFactor() : float{
		return 0.8; //???
	}

	public function onEntityLand(Entity $entity) : ?float{
		if($entity instanceof Living && $entity->isSneaking()){
			return null;
		}
		$entity->resetFallDistance();
		return -$entity->getMotion()->y;
	}

	//TODO: slime blocks should slow entities walking on them to about 0.4x original speed
}
}

namespace pocketmine\block {
use pocketmine\block\inventory\SmithingTableInventory;
use pocketmine\item\Item;
use pocketmine\math\Vector3;
use pocketmine\player\Player;

final class SmithingTable extends Opaque{

	public function onInteract(Item $item, int $face, Vector3 $clickVector, ?Player $player = null, array &$returnedItems = []) : bool{
		if($player !== null){
			$player->setCurrentWindow(new SmithingTableInventory($this->position));
		}

		return true;
	}

	public function getFuelTime() : int{
		return 300;
	}
}
}

namespace pocketmine\block {
use pocketmine\item\Item;
use pocketmine\item\VanillaItems;

class Snow extends Opaque{

	public function getDropsForCompatibleTool(Item $item) : array{
		return [
			VanillaItems::SNOWBALL()->setCount(4)
		];
	}

	public function isAffectedBySilkTouch() : bool{
		return true;
	}
}
}

namespace pocketmine\block {
use pocketmine\math\AxisAlignedBB;
use pocketmine\math\Facing;

class SoulSand extends Opaque{

	protected function recalculateCollisionBoxes() : array{
		return [AxisAlignedBB::one()->trim(Facing::UP, 1 / 8)];
	}
}
}

namespace pocketmine\block {
use pocketmine\data\runtime\RuntimeDataDescriber;

class Sponge extends Opaque{
	protected bool $wet = false;

	public function describeBlockItemState(RuntimeDataDescriber $w) : void{
		$w->bool($this->wet);
	}

	public function isWet() : bool{ return $this->wet; }

	public function setWet(bool $wet) : self{
		$this->wet = $wet;
		return $this;
	}
}
}

namespace pocketmine\block {
use pocketmine\block\utils\Colored;
use pocketmine\block\utils\ColoredTrait;

final class StainedHardenedClay extends HardenedClay implements Colored{
	use ColoredTrait;
}
}

namespace pocketmine\block {
use pocketmine\block\inventory\StonecutterInventory;
use pocketmine\block\utils\FacesOppositePlacingPlayerTrait;
use pocketmine\block\utils\HorizontalFacing;
use pocketmine\block\utils\SupportType;
use pocketmine\item\Item;
use pocketmine\math\AxisAlignedBB;
use pocketmine\math\Facing;
use pocketmine\math\Vector3;
use pocketmine\player\Player;

class Stonecutter extends Transparent implements HorizontalFacing{
	use FacesOppositePlacingPlayerTrait;

	public function onInteract(Item $item, int $face, Vector3 $clickVector, ?Player $player = null, array &$returnedItems = []) : bool{
		if($player !== null){
			$player->setCurrentWindow(new StonecutterInventory($this->position));
		}
		return true;
	}

	protected function recalculateCollisionBoxes() : array{
		return [AxisAlignedBB::one()->trim(Facing::UP, 7 / 16)];
	}

	public function getSupportType(int $facing) : SupportType{
		return SupportType::NONE;
	}
}
}

namespace pocketmine\block {
final class StructureVoid extends Transparent{

	protected function recalculateCollisionBoxes() : array{
		return [];
	}

	public function isSolid() : bool{
		return false;
	}
}
}

namespace pocketmine\block {
use pocketmine\block\utils\RedstoneSignalEmitter;

class TrappedChest extends Chest implements RedstoneSignalEmitter{

	//TODO: Redstone!

}
}

namespace pocketmine\block {
class UnderwaterTorch extends Torch{

	public function canBeFlowedInto() : bool{
		return false;
	}
}
}

namespace pocketmine\block {
use pocketmine\block\utils\HorizontalFacing;
use pocketmine\block\utils\HorizontalFacingTrait;
use pocketmine\item\Item;
use pocketmine\math\Axis;
use pocketmine\math\Facing;
use pocketmine\math\Vector3;
use pocketmine\player\Player;
use pocketmine\world\BlockTransaction;

final class WallBanner extends BaseBanner implements HorizontalFacing{
	use HorizontalFacingTrait;

	protected function getOminousVersion() : Block{
		return VanillaBlocks::OMINOUS_WALL_BANNER()->setFacing($this->facing);
	}

	protected function getSupportingFace() : int{
		return Facing::opposite($this->facing);
	}

	public function place(BlockTransaction $tx, Item $item, Block $blockReplace, Block $blockClicked, int $face, Vector3 $clickVector, ?Player $player = null) : bool{
		if(Facing::axis($face) === Axis::Y){
			return false;
		}
		$this->facing = $face;
		return parent::place($tx, $item, $blockReplace, $blockClicked, $face, $clickVector, $player);
	}
}
}

namespace pocketmine\block {
use pocketmine\block\utils\HorizontalFacing;
use pocketmine\block\utils\HorizontalFacingTrait;
use pocketmine\data\runtime\RuntimeDataDescriber;
use pocketmine\item\Item;
use pocketmine\item\VanillaItems;
use pocketmine\math\Axis;
use pocketmine\math\Facing;
use pocketmine\math\Vector3;
use pocketmine\player\Player;
use pocketmine\world\BlockTransaction;

final class WallCoralFan extends BaseCoral implements HorizontalFacing{
	use HorizontalFacingTrait;

	protected function describeBlockOnlyState(RuntimeDataDescriber $w) : void{
		$w->horizontalFacing($this->facing);
	}

	public function place(BlockTransaction $tx, Item $item, Block $blockReplace, Block $blockClicked, int $face, Vector3 $clickVector, ?Player $player = null) : bool{
		$axis = Facing::axis($face);
		if(($axis !== Axis::X && $axis !== Axis::Z) || !$this->canBeSupportedAt($blockReplace, Facing::opposite($face))){
			return false;
		}
		$this->facing = $face;

		$this->dead = !$this->isCoveredWithWater();

		return parent::place($tx, $item, $blockReplace, $blockClicked, $face, $clickVector, $player);
	}

	public function onNearbyBlockChange() : void{
		$world = $this->position->getWorld();
		if(!$this->canBeSupportedAt($this, Facing::opposite($this->facing))){
			$world->useBreakOn($this->position);
		}else{
			parent::onNearbyBlockChange();
		}
	}

	private function canBeSupportedAt(Block $block, int $face) : bool{
		return $block->getAdjacentSupportType($face)->hasCenterSupport();
	}

	public function asItem() : Item{
		return VanillaItems::CORAL_FAN()->setCoralType($this->coralType)->setDead($this->dead);
	}
}
}

namespace pocketmine\block {
use pocketmine\block\utils\HorizontalFacing;
use pocketmine\block\utils\HorizontalFacingTrait;
use pocketmine\block\utils\SupportType;
use pocketmine\item\Item;
use pocketmine\math\Axis;
use pocketmine\math\AxisAlignedBB;
use pocketmine\math\Facing;
use pocketmine\math\Vector3;
use pocketmine\player\Player;
use pocketmine\utils\AssumptionFailedError;
use pocketmine\world\BlockTransaction;

final class WallHangingSign extends BaseSign implements HorizontalFacing{
	use HorizontalFacingTrait;

	protected function getSupportingFace() : int{
		return Facing::rotateY($this->facing, clockwise: true);
	}

	public function onNearbyBlockChange() : void{
		//NOOP - disable default self-destruct behaviour
	}

	protected function recalculateCollisionBoxes() : array{
		//only the cross bar is collidable
		return [AxisAlignedBB::one()->trim(Facing::DOWN, 7 / 8)->squash(Facing::axis($this->facing), 3 / 4)];
	}

	public function place(BlockTransaction $tx, Item $item, Block $blockReplace, Block $blockClicked, int $face, Vector3 $clickVector, ?Player $player = null) : bool{
		if($player === null){
			return false;
		}
		$attachFace = Facing::axis($face) === Axis::Y ? Facing::rotateY($player->getHorizontalFacing(), clockwise: true) : $face;

		if($this->canBeSupportedAt($blockReplace->getSide($attachFace), $attachFace)){
			$direction = $attachFace;
		}elseif($this->canBeSupportedAt($blockReplace->getSide($opposite = Facing::opposite($attachFace)), $opposite)){
			$direction = $opposite;
		}else{
			return false;
		}

		$this->facing = Facing::rotateY(Facing::opposite($direction), clockwise: true);
		//the front should always face the player if possible
		if($this->facing === $player->getHorizontalFacing()){
			$this->facing = Facing::opposite($this->facing);
		}

		return parent::place($tx, $item, $blockReplace, $blockClicked, $face, $clickVector, $player);
	}

	private function canBeSupportedAt(Block $block, int $face) : bool{
		return
			($block instanceof WallHangingSign && Facing::axis(Facing::rotateY($block->getFacing(), clockwise: true)) === Facing::axis($face)) ||
			$block->getSupportType(Facing::opposite($face)) === SupportType::FULL;
	}

	protected function getFacingDegrees() : float{
		return match($this->facing){
			Facing::SOUTH => 0,
			Facing::WEST => 90,
			Facing::NORTH => 180,
			Facing::EAST => 270,
			default => throw new AssumptionFailedError("Invalid facing direction: " . $this->facing),
		};
	}
}
}

namespace pocketmine\block {
use pocketmine\block\utils\HorizontalFacing;
use pocketmine\block\utils\HorizontalFacingTrait;
use pocketmine\item\Item;
use pocketmine\math\Axis;
use pocketmine\math\Facing;
use pocketmine\math\Vector3;
use pocketmine\player\Player;
use pocketmine\utils\AssumptionFailedError;
use pocketmine\world\BlockTransaction;

final class WallSign extends BaseSign implements HorizontalFacing{
	use HorizontalFacingTrait;

	protected function getSupportingFace() : int{
		return Facing::opposite($this->facing);
	}

	public function place(BlockTransaction $tx, Item $item, Block $blockReplace, Block $blockClicked, int $face, Vector3 $clickVector, ?Player $player = null) : bool{
		if(Facing::axis($face) === Axis::Y){
			return false;
		}
		$this->facing = $face;
		return parent::place($tx, $item, $blockReplace, $blockClicked, $face, $clickVector, $player);
	}

	protected function getHitboxCenter() : Vector3{
		[$xOffset, $zOffset] = match($this->facing){
			Facing::NORTH => [0, 15 / 16],
			Facing::SOUTH => [0, 1 / 16],
			Facing::WEST => [15 / 16, 0],
			Facing::EAST => [1 / 16, 0],
			default => throw new AssumptionFailedError("Invalid facing direction: " . $this->facing),
		};
		return $this->position->add($xOffset, 0.5, $zOffset);
	}

	protected function getFacingDegrees() : float{
		return match($this->facing){
			Facing::SOUTH => 0,
			Facing::WEST => 90,
			Facing::NORTH => 180,
			Facing::EAST => 270,
			default => throw new AssumptionFailedError("Invalid facing direction: " . $this->facing),
		};
	}
}
}

namespace pocketmine\block {
use pocketmine\block\utils\Colored;
use pocketmine\block\utils\ColoredTrait;

class Wool extends Opaque implements Colored{
	use ColoredTrait;

	public function getFlameEncouragement() : int{
		return 30;
	}

	public function getFlammability() : int{
		return 60;
	}
}
}

