<?php

/*
 *
 *  ____            _        _   __  __ _                  __  __ ____
 * |  _ \ ___   ___| | _____| |_|  \/  (_)_ __   ___      |  \/  |  _ \
 * | |_) / _ \ / __| |/ / _ \ __| |\/| | | '_ \ / _ \_____| |\/| | |_) |
 * |  __/ (_) | (__|   <  __/ |_| |  | | | | | |  __/_____| |  | |  __/
 * |_|   \___/ \___|_|\_\___|\__|_|  |_|_|_| |_|\___|     |_|  |_|_|
 *
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU Lesser General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 *
 * @author PocketMine Team
 * @link http://www.pocketmine.net/
 *
 *
 */

declare(strict_types=1);

namespace pocketmine\item;

use pocketmine\block\Block;
use pocketmine\block\BlockToolType;
use pocketmine\block\BlockTypeIds;
use pocketmine\block\VanillaBlocks;
use pocketmine\entity\Entity;
use pocketmine\entity\Location;
use pocketmine\entity\projectile\Trident as TridentEntity;
use pocketmine\event\entity\ProjectileLaunchEvent;
use pocketmine\math\Vector3;
use pocketmine\player\Player;
use pocketmine\world\sound\FlintSteelSound;
use pocketmine\world\sound\TridentThrowSound;
use function class_exists;
use function method_exists;
use function min;

class Sword extends TieredTool{

	public function getBlockToolType() : int{
		return BlockToolType::SWORD;
	}

	public function getAttackPoints() : int{
		return $this->tier->getBaseAttackPoints();
	}

	public function getBlockToolHarvestLevel() : int{
		return 1;
	}

	public function getMiningEfficiency(bool $isCorrectTool) : float{
		return parent::getMiningEfficiency($isCorrectTool) * 1.5; //swords break any block 1.5x faster than hand
	}

	protected function getBaseMiningEfficiency() : float{
		return 10;
	}

	public function onDestroyBlock(Block $block, array &$returnedItems) : bool{
		if(!$block->getBreakInfo()->breaksInstantly()){
			return $this->applyDamage(2);
		}
		return false;
	}

	public function onAttackEntity(Entity $victim, array &$returnedItems) : bool{
		return $this->applyDamage(1);
	}
}

class Axe extends TieredTool{

	public function getBlockToolType() : int{
		return BlockToolType::AXE;
	}

	public function getBlockToolHarvestLevel() : int{
		return $this->tier->getHarvestLevel();
	}

	public function getAttackPoints() : int{
		return $this->tier->getBaseAttackPoints() - 1;
	}

	public function onDestroyBlock(Block $block, array &$returnedItems) : bool{
		if(!$block->getBreakInfo()->breaksInstantly()){
			return $this->applyDamage(1);
		}
		return false;
	}

	public function onAttackEntity(Entity $victim, array &$returnedItems) : bool{
		return $this->applyDamage(2);
	}
}

class Pickaxe extends TieredTool{

	public function getBlockToolType() : int{
		return BlockToolType::PICKAXE;
	}

	public function getBlockToolHarvestLevel() : int{
		return $this->tier->getHarvestLevel();
	}

	public function getAttackPoints() : int{
		return $this->tier->getBaseAttackPoints() - 2;
	}

	public function onDestroyBlock(Block $block, array &$returnedItems) : bool{
		if(!$block->getBreakInfo()->breaksInstantly()){
			return $this->applyDamage(1);
		}
		return false;
	}

	public function onAttackEntity(Entity $victim, array &$returnedItems) : bool{
		return $this->applyDamage(2);
	}
}

class Shovel extends TieredTool{

	public function getBlockToolType() : int{
		return BlockToolType::SHOVEL;
	}

	public function getBlockToolHarvestLevel() : int{
		return $this->tier->getHarvestLevel();
	}

	public function getAttackPoints() : int{
		return $this->tier->getBaseAttackPoints() - 3;
	}

	public function onDestroyBlock(Block $block, array &$returnedItems) : bool{
		if(!$block->getBreakInfo()->breaksInstantly()){
			return $this->applyDamage(1);
		}
		return false;
	}

	public function onAttackEntity(Entity $victim, array &$returnedItems) : bool{
		return $this->applyDamage(2);
	}
}

class Hoe extends TieredTool{

	public function getBlockToolType() : int{
		return BlockToolType::HOE;
	}

	public function onAttackEntity(Entity $victim, array &$returnedItems) : bool{
		return $this->applyDamage(1);
	}

	public function onDestroyBlock(Block $block, array &$returnedItems) : bool{
		if(!$block->getBreakInfo()->breaksInstantly()){
			return $this->applyDamage(1);
		}
		return false;
	}
}

class Shears extends Tool{

	public function getMaxDurability() : int{
		return 239;
	}

	public function getBlockToolType() : int{
		return BlockToolType::SHEARS;
	}

	public function getBlockToolHarvestLevel() : int{
		return 1;
	}

	protected function getBaseMiningEfficiency() : float{
		return 15;
	}

	public function onDestroyBlock(Block $block, array &$returnedItems) : bool{
		return $this->applyDamage(1);
	}
}

class FlintSteel extends Tool{

	public function onInteractBlock(Player $player, Block $blockReplace, Block $blockClicked, int $face, Vector3 $clickVector, array &$returnedItems) : ItemUseResult{
		if($blockReplace->getTypeId() === BlockTypeIds::AIR){
			$world = $player->getWorld();
			$world->setBlock($blockReplace->getPosition(), VanillaBlocks::FIRE());
			if(class_exists(FlintSteelSound::class)){
				$world->addSound($blockReplace->getPosition()->add(0.5, 0.5, 0.5), new FlintSteelSound());
			}

			$this->applyDamage(1);

			return ItemUseResult::SUCCESS;
		}

		return ItemUseResult::NONE;
	}

	public function getMaxDurability() : int{
		return 65;
	}
}

class FishingRod extends Tool{

	public function getMaxStackSize() : int{
		return 1;
	}

	public function getMaxDurability() : int{
		return 384;
	}
}

class Trident extends Tool implements Releasable{
	private const DAMAGE_ON_THROW = 1;

	public function getMaxDurability() : int{
		return 251;
	}

	public function onReleaseUsing(Player $player, array &$returnedItems) : ItemUseResult{
		$location = $player->getLocation();

		$diff = method_exists($player, 'getItemUseDuration') ? $player->getItemUseDuration() : 20;
		if($diff < 14){
			return ItemUseResult::FAIL;
		}

		$item = $this->pop();
		$hasFinite = method_exists($player, 'hasFiniteResources') ? $player->hasFiniteResources() : (method_exists($player, 'isCreative') ? !$player->isCreative() : true);
		if($hasFinite){
			$item->applyDamage(self::DAMAGE_ON_THROW);
		}
		if($item->isNull()){
			//canStartUsingItem() will normally prevent this, but it's possible the item might've been modified between
			//the start action and the release, so it's best to account for this anyway
			return ItemUseResult::FAIL;
		}

		$launchLocation = Location::fromObject(
			$player->getEyePos(),
			$player->getWorld(),
			($location->yaw > 180 ? 360 : 0) - $location->yaw,
			-$location->pitch
		);

		$entity = null;
		if(class_exists(TridentEntity::class)){
			$entity = new TridentEntity($launchLocation, $item, $player);
		}elseif(class_exists(\pocketmine\entity\projectile\ThrownTrident::class)){
			$entity = new \pocketmine\entity\projectile\ThrownTrident($launchLocation, $player);
		}

		if($entity === null){
			return ItemUseResult::FAIL;
		}

		$p = $diff / 20;
		$baseForce = min((($p ** 2) + $p * 2) / 3, 1) * 2.4;
		$entity->setMotion($player->getDirectionVector()->multiply($baseForce));

		if(class_exists(ProjectileLaunchEvent::class)){
			$ev = new ProjectileLaunchEvent($entity);
			$ev->call();
			if($ev->isCancelled()){
				if(method_exists($ev->getEntity(), 'flagForDespawn')){
					$ev->getEntity()->flagForDespawn();
				}elseif(method_exists($ev->getEntity(), 'close')){
					$ev->getEntity()->close();
				}
				return ItemUseResult::FAIL;
			}
		}

		$entity->spawnToAll();

		if(class_exists(TridentThrowSound::class)){
			$location->getWorld()->addSound($location, new TridentThrowSound());
		}

		return ItemUseResult::SUCCESS;
	}

	public function getAttackPoints() : int{
		return 9;
	}

	public function canStartUsingItem(Player $player) : bool{
		return $this->damage < $this->getMaxDurability() - self::DAMAGE_ON_THROW;
	}

	public function onAttackEntity(Entity $victim, array &$returnedItems) : bool{
		return $this->applyDamage(1);
	}

	public function onDestroyBlock(Block $block, array &$returnedItems) : bool{
		if(!$block->getBreakInfo()->breaksInstantly()){
			return $this->applyDamage(2);
		}
		return false;
	}
}
