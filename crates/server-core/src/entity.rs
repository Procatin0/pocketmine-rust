//! Entity model translated from PocketMine-MP (`src/entity/Entity.php`,
//! `object/ItemEntity.php`, `object/FallingBlock.php`).
//!
//! Only the base machinery plus the two object entities this server can
//! produce today are ported: item drops (`minecraft:item`) and falling blocks
//! (`minecraft:falling_block`). Movement follows `Entity::move()` (per-axis
//! AABB sweep against block collision boxes), `Entity::tryChangeMovement()`
//! (drag then gravity), `Entity::checkGroundState()` and
//! `Entity::updateFallState()`.

use bedrock_protocol::packets::set_actor_data::{MetadataEntry, MetadataValue};
use bedrock_protocol::types::Vector3f;
use bedrock_protocol::ItemStack;
use bedrock_world::{block_id::AIR, block_states};

use crate::physics;

/// `Entity::MOTION_THRESHOLD`: components below this are snapped to zero.
pub const MOTION_THRESHOLD: f32 = 0.00001;
/// `ItemEntity::DEFAULT_DESPAWN_DELAY` (5 minutes).
pub const ITEM_DESPAWN_TICKS: i32 = 6000;
/// `ItemEntity::MERGE_CHECK_PERIOD`.
pub const ITEM_MERGE_PERIOD: i32 = 2;
/// `World::dropItem()`'s default pickup delay.
pub const ITEM_PICKUP_DELAY: i32 = 10;
/// `EntityMetadataFlags::HAS_COLLISION` / `AFFECTED_BY_GRAVITY`.
pub const FLAG_HAS_COLLISION: i64 = 1 << 48;
pub const FLAG_AFFECTED_BY_GRAVITY: i64 = 1 << 49;

/// `EntitySizeInfo`: height, width and the derived eye height.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EntitySize {
    pub height: f32,
    pub width: f32,
}

impl EntitySize {
    pub fn new(height: f32, width: f32) -> Self {
        Self { height, width }
    }

    /// `min($height / 2 + 0.1, $height)`.
    pub fn eye_height(&self) -> f32 {
        (self.height / 2.0 + 0.1).min(self.height)
    }
}

/// Axis-aligned bounding box, `pocketmine\math\AxisAlignedBB`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Aabb {
    pub min_x: f32,
    pub min_y: f32,
    pub min_z: f32,
    pub max_x: f32,
    pub max_y: f32,
    pub max_z: f32,
}

impl Aabb {
    pub fn new(min_x: f32, min_y: f32, min_z: f32, max_x: f32, max_y: f32, max_z: f32) -> Self {
        Self {
            min_x,
            min_y,
            min_z,
            max_x,
            max_y,
            max_z,
        }
    }

    /// `Entity::recalculateBoundingBox()`: centred on x/z, from the feet up.
    pub fn from_position(position: Vector3f, size: EntitySize) -> Self {
        let half = size.width / 2.0;
        Self::new(
            position.x - half,
            position.y,
            position.z - half,
            position.x + half,
            position.y + size.height,
            position.z + half,
        )
    }

    pub fn add_coord(&self, dx: f32, dy: f32, dz: f32) -> Self {
        Self::new(
            self.min_x + dx,
            self.min_y + dy,
            self.min_z + dz,
            self.max_x + dx,
            self.max_y + dy,
            self.max_z + dz,
        )
    }

    pub fn offset(&mut self, dx: f32, dy: f32, dz: f32) {
        self.min_x += dx;
        self.max_x += dx;
        self.min_y += dy;
        self.max_y += dy;
        self.min_z += dz;
        self.max_z += dz;
    }

    /// `AxisAlignedBB::calculateYOffset()`, unchanged.
    pub fn calculate_y_offset(&self, moving: &Aabb, mut dy: f32) -> f32 {
        if self.max_x <= moving.min_x || self.min_x >= moving.max_x {
            return dy;
        }
        if self.max_z <= moving.min_z || self.min_z >= moving.max_z {
            return dy;
        }
        if dy > 0.0 && moving.max_y <= self.min_y {
            let d = self.min_y - moving.max_y;
            if d < dy {
                dy = d;
            }
        } else if dy < 0.0 && moving.min_y >= self.max_y {
            let d = self.max_y - moving.min_y;
            if d > dy {
                dy = d;
            }
        }
        dy
    }

    /// `AxisAlignedBB::calculateXOffset()`.
    pub fn calculate_x_offset(&self, moving: &Aabb, mut dx: f32) -> f32 {
        if self.max_y <= moving.min_y || self.min_y >= moving.max_y {
            return dx;
        }
        if self.max_z <= moving.min_z || self.min_z >= moving.max_z {
            return dx;
        }
        if dx > 0.0 && moving.max_x <= self.min_x {
            let d = self.min_x - moving.max_x;
            if d < dx {
                dx = d;
            }
        } else if dx < 0.0 && moving.min_x >= self.max_x {
            let d = self.max_x - moving.min_x;
            if d > dx {
                dx = d;
            }
        }
        dx
    }

    /// `AxisAlignedBB::calculateZOffset()`.
    pub fn calculate_z_offset(&self, moving: &Aabb, mut dz: f32) -> f32 {
        if self.max_x <= moving.min_x || self.min_x >= moving.max_x {
            return dz;
        }
        if self.max_y <= moving.min_y || self.min_y >= moving.max_y {
            return dz;
        }
        if dz > 0.0 && moving.max_z <= self.min_z {
            let d = self.min_z - moving.max_z;
            if d < dz {
                dz = d;
            }
        } else if dz < 0.0 && moving.min_z >= self.max_z {
            let d = self.max_z - moving.min_z;
            if d > dz {
                dz = d;
            }
        }
        dz
    }

    pub fn intersects(&self, other: &Aabb) -> bool {
        self.min_x < other.max_x
            && self.max_x > other.min_x
            && self.min_y < other.max_y
            && self.max_y > other.min_y
            && self.min_z < other.max_z
            && self.max_z > other.min_z
    }
}

/// What an entity is the simulation of.
#[derive(Debug, Clone, PartialEq)]
pub enum EntityKind {
    Item {
        item: ItemStack,
        /// `ItemEntity::$pickupDelay`.
        pickup_delay: i32,
        /// `ItemEntity::$despawnDelay` (`-1` never despawns).
        despawn_delay: i32,
        /// `ItemEntity::$owner`: the player that dropped it, if any.
        owner: String,
    },
    FallingBlock {
        block_id: u32,
    },
}

/// One live entity. `unique_id` is `Entity::getId()`, `runtime_id` the
/// Bedrock runtime id used by every actor packet.
#[derive(Debug, Clone, PartialEq)]
pub struct Entity {
    pub unique_id: i64,
    pub runtime_id: u64,
    pub kind: EntityKind,
    pub position: Vector3f,
    pub motion: Vector3f,
    pub yaw: f32,
    pub pitch: f32,
    pub head_yaw: f32,
    pub on_ground: bool,
    pub fall_distance: f32,
    /// `Entity::onHitGround()` runs before `resetFallDistance()` in
    /// PocketMine; the value is kept here so the landing tick can still use
    /// it.
    pub landed_fall_distance: f32,
    /// `Entity::$ticksLived`.
    pub ticks_lived: u32,
    /// `Entity::isClosed()`: already despawned, must not tick again.
    pub closed: bool,
}

impl Entity {
    /// `new ItemEntity(...)`: a 0.25×0.25 box at `position`.
    ///
    /// `position` is the feet position (`Location`), not the block centre.
    pub fn item(
        unique_id: i64,
        runtime_id: u64,
        position: Vector3f,
        item: ItemStack,
        motion: Vector3f,
        owner: impl Into<String>,
    ) -> Self {
        Self {
            unique_id,
            runtime_id,
            kind: EntityKind::Item {
                item,
                pickup_delay: ITEM_PICKUP_DELAY,
                despawn_delay: ITEM_DESPAWN_TICKS,
                owner: owner.into(),
            },
            position,
            motion,
            yaw: 0.0,
            pitch: 0.0,
            head_yaw: 0.0,
            on_ground: false,
            fall_distance: 0.0,
            landed_fall_distance: 0.0,
            ticks_lived: 0,
            closed: false,
        }
    }

    /// `new FallingBlock(Location::fromObject($pos->add(0.5, 0, 0.5)), $block)`.
    pub fn falling_block(
        unique_id: i64,
        runtime_id: u64,
        position: Vector3f,
        block_id: u32,
    ) -> Self {
        Self {
            unique_id,
            runtime_id,
            kind: EntityKind::FallingBlock { block_id },
            position,
            motion: Vector3f::new(0.0, 0.0, 0.0),
            yaw: 0.0,
            pitch: 0.0,
            head_yaw: 0.0,
            on_ground: false,
            fall_distance: 0.0,
            landed_fall_distance: 0.0,
            ticks_lived: 0,
            closed: false,
        }
    }

    /// `getInitialSizeInfo()`: `ItemEntity` 0.25/0.25, `FallingBlock` 0.98/0.98.
    pub fn size(&self) -> EntitySize {
        match self.kind {
            EntityKind::Item { .. } => EntitySize::new(0.25, 0.25),
            EntityKind::FallingBlock { .. } => EntitySize::new(0.98, 0.98),
        }
    }

    /// `getNetworkTypeId()`: `EntityIds::ITEM` / `EntityIds::FALLING_BLOCK`.
    pub fn network_type_id(&self) -> &'static str {
        match self.kind {
            EntityKind::Item { .. } => "minecraft:item",
            EntityKind::FallingBlock { .. } => "minecraft:falling_block",
        }
    }

    /// `getInitialGravity()`.
    pub fn gravity(&self) -> f32 {
        0.04
    }

    /// `getInitialDragMultiplier()`.
    pub fn drag(&self) -> f32 {
        0.02
    }

    /// `ItemEntity::applyDragBeforeGravity()`.
    pub fn drag_before_gravity(&self) -> bool {
        matches!(self.kind, EntityKind::Item { .. })
    }

    pub fn bounding_box(&self) -> Aabb {
        Aabb::from_position(self.position, self.size())
    }

    /// `Entity::getOffsetPosition()`: items render 0.125 up, falling blocks
    /// 0.49 up (their box is centred in the block cell).
    pub fn offset_position(&self) -> Vector3f {
        let dy = match self.kind {
            EntityKind::Item { .. } => 0.125,
            EntityKind::FallingBlock { .. } => 0.49,
        };
        Vector3f::new(self.position.x, self.position.y + dy, self.position.z)
    }

    /// `Entity::syncNetworkData()` plus the per-entity variant, i.e.
    /// `ItemEntity`'s inherited set and `FallingBlock`'s `VARIANT`.
    pub fn metadata(&self) -> Vec<MetadataEntry> {
        let size = self.size();
        let mut metadata = vec![
            MetadataEntry::new(
                0,
                MetadataValue::Long(FLAG_HAS_COLLISION | FLAG_AFFECTED_BY_GRAVITY),
            ),
            MetadataEntry::new(3, MetadataValue::Byte(0)),
            MetadataEntry::new(4, MetadataValue::String(String::new())),
            MetadataEntry::new(5, MetadataValue::Long(-1)),
            MetadataEntry::new(6, MetadataValue::Long(0)),
            MetadataEntry::new(37, MetadataValue::Long(-1)),
            MetadataEntry::new(38, MetadataValue::Float(1.0)),
            MetadataEntry::new(53, MetadataValue::Float(size.width)),
            MetadataEntry::new(54, MetadataValue::Float(size.height)),
        ];
        if let EntityKind::FallingBlock { block_id } = self.kind {
            metadata.push(MetadataEntry::new(
                2,
                MetadataValue::Int(block_id as i32),
            ));
        }
        metadata
    }

    /// `Entity::onUpdate()` for one tick: movement, then `entityBaseTick`.
    pub fn on_update(&mut self, world: &crate::world_state::WorldState) {
        if self.closed {
            return;
        }
        // `tryChangeMovement()`.
        let mut friction = 1.0 - self.drag();
        let mut motion_y = self.motion.y;
        if self.drag_before_gravity() {
            motion_y *= friction;
        }
        motion_y -= self.gravity();
        if !self.drag_before_gravity() {
            motion_y *= friction;
        }
        let motion_x = self.motion.x;
        let motion_z = self.motion.z;
        if self.on_ground {
            // `Block::getFrictionFactor()`: 0.6 for every block this server
            // models (no ice/slime friction table yet).
            friction *= 0.6;
        }
        self.motion = Vector3f::new(motion_x * friction, motion_y, motion_z * friction);

        if self.motion.x.abs() <= MOTION_THRESHOLD {
            self.motion.x = 0.0;
        }
        if self.motion.y.abs() <= MOTION_THRESHOLD {
            self.motion.y = 0.0;
        }
        if self.motion.z.abs() <= MOTION_THRESHOLD {
            self.motion.z = 0.0;
        }
        if self.motion.x != 0.0 || self.motion.y != 0.0 || self.motion.z != 0.0 {
            let (dx, dy, dz) = (self.motion.x, self.motion.y, self.motion.z);
            self.move_entity(world, dx, dy, dz);
        }

        self.entity_base_tick(world);
    }

    /// `Entity::entityBaseTick()`: void damage (despawn below the world) and
    /// the tick counter. Item despawn/pickup delays live in [`Self::item_tick`].
    fn entity_base_tick(&mut self, _world: &crate::world_state::WorldState) {
        self.ticks_lived = self.ticks_lived.saturating_add(1);
        if self.position.y <= crate::world_state::WORLD_MIN_Y as f32 - 16.0 {
            self.closed = true;
        }
    }

    /// `ItemEntity::entityBaseTick()`: pickup delay, then despawn delay.
    pub fn item_tick(&mut self) {
        let EntityKind::Item {
            pickup_delay,
            despawn_delay,
            ..
        } = &mut self.kind
        else {
            return;
        };
        if *pickup_delay > 0 {
            *pickup_delay -= 1;
        }
        if *despawn_delay != -1 {
            *despawn_delay -= 1;
            if *despawn_delay <= 0 {
                self.closed = true;
            }
        }
    }

    pub fn pickup_delay(&self) -> i32 {
        match &self.kind {
            EntityKind::Item { pickup_delay, .. } => *pickup_delay,
            _ => 0,
        }
    }

    /// `ItemEntity::setPickupDelay()`; `-1` means never pickable.
    pub fn set_pickup_delay(&mut self, delay: i32) {
        if let EntityKind::Item { pickup_delay, .. } = &mut self.kind {
            *pickup_delay = delay;
        }
    }

    /// `Entity::move()` without the step-height branch: these entities have a
    /// step height of 0, so the sweep reduces to Y, X, Z resolution.
    pub fn move_entity(
        &mut self,
        world: &crate::world_state::WorldState,
        dx: f32,
        dy: f32,
        dz: f32,
    ) {
        let wanted_x = dx;
        let wanted_y = dy;
        let wanted_z = dz;

        let mut moving = self.bounding_box();
        let boxes = world.collision_boxes(&moving.add_coord(dx, dy, dz));

        let mut dy = dy;
        for bb in &boxes {
            dy = bb.calculate_y_offset(&moving, dy);
        }
        moving.offset(0.0, dy, 0.0);

        let mut dx = dx;
        for bb in &boxes {
            dx = bb.calculate_x_offset(&moving, dx);
        }
        moving.offset(dx, 0.0, 0.0);

        let mut dz = dz;
        for bb in &boxes {
            dz = bb.calculate_z_offset(&moving, dz);
        }
        moving.offset(0.0, 0.0, dz);

        self.position = Vector3f::new(
            (moving.min_x + moving.max_x) / 2.0,
            moving.min_y,
            (moving.min_z + moving.max_z) / 2.0,
        );

        // `checkGroundState()`: only a downward block counts as ground.
        self.on_ground = wanted_y != dy && wanted_y < 0.0;

        // `updateFallState()`.
        if dy < self.fall_distance {
            self.fall_distance -= dy;
        } else {
            self.fall_distance = 0.0;
        }
        if self.on_ground && self.fall_distance > 0.0 {
            self.landed_fall_distance = self.fall_distance;
            self.fall_distance = 0.0;
        }

        // `$this->motion = withComponents($wanted !== $got ? 0 : null, ...)`.
        if wanted_x != dx {
            self.motion.x = 0.0;
        }
        if wanted_y != dy {
            self.motion.y = 0.0;
        }
        if wanted_z != dz {
            self.motion.z = 0.0;
        }
    }

    /// `FallingBlock::entityBaseTick()` target cell: the block cell the entity
    /// is occupying, `location + (-width/2, height, -width/2)` floored.
    pub fn falling_block_cell(&self) -> (i32, i32, i32) {
        let size = self.size();
        (
            (self.position.x - size.width / 2.0).floor() as i32,
            (self.position.y + size.height).floor() as i32,
            (self.position.z - size.width / 2.0).floor() as i32,
        )
    }
}

/// `World::dropItem()`'s item for a broken block: `$block->asItem()`, i.e.
/// the block's own item form (`data/block_id_to_item_id_map.json`).
pub fn block_item(block_id: u32) -> Option<ItemStack> {
    let name = block_states::state_name(block_id)?;
    let item_name = block_item_name(name)?;
    let network_id = crate::inventory::item_network_id(item_name)?;
    Some(ItemStack {
        network_id,
        count: 1,
        ..ItemStack::empty()
    })
}

fn block_item_name(name: &str) -> Option<&'static str> {
    static MAP: std::sync::OnceLock<std::collections::HashMap<&'static str, &'static str>> =
        std::sync::OnceLock::new();
    let map = MAP.get_or_init(|| {
        let raw: std::collections::HashMap<String, String> =
            serde_json::from_str(include_str!("../../../data/block_id_to_item_id_map.json"))
                .unwrap_or_default();
        raw.into_iter()
            .map(|(block, item)| {
                (
                    Box::leak(block.into_boxed_str()) as &'static str,
                    Box::leak(item.into_boxed_str()) as &'static str,
                )
            })
            .collect()
    });
    map.get(name)
        .copied()
        // `state_name()` may or may not carry the namespace depending on the
        // table; try both spellings before giving up.
        .or_else(|| map.get(format!("minecraft:{name}").as_str()).copied())
}

/// `ItemEntity::isMergeCandidate()` + `isMergeable()` + `tryMergeInto()`:
/// merges the smaller stack into the bigger one when both fit in one stack.
pub fn try_merge(source: &mut Entity, consumer: &mut Entity) {
    let (
        EntityKind::Item {
            item: source_item,
            pickup_delay: source_pickup,
            despawn_delay: source_despawn,
            ..
        },
        EntityKind::Item {
            item: consumer_item,
            pickup_delay: consumer_pickup,
            despawn_delay: consumer_despawn,
            ..
        },
    ) = (&mut source.kind, &mut consumer.kind)
    else {
        return;
    };
    if *source_pickup == -1 || *consumer_pickup == -1 {
        return;
    }
    let max = crate::inventory::max_stack_size(consumer_item.network_id);
    if source_item.network_id != consumer_item.network_id
        || source_item.meta != consumer_item.meta
        || source_item.nbt != consumer_item.nbt
        || source_item.count as u32 + consumer_item.count as u32 > max as u32
    {
        return;
    }
    consumer_item.count += source_item.count;
    source.closed = true;
    *consumer_pickup = (*consumer_pickup).max(*source_pickup);
    if *source_despawn != -1 && *consumer_despawn != -1 {
        *consumer_despawn = (*consumer_despawn).max(*source_despawn);
    }
}

/// `World::getBlockCollisionBoxes()`: the one-block boxes of every solid
/// block intersected by `box`.
pub fn collision_boxes_in(
    world: &crate::world_state::WorldState,
    box_: &Aabb,
) -> Vec<Aabb> {
    let mut out = Vec::new();
    let min_x = box_.min_x.floor() as i32;
    let min_y = box_.min_y.floor() as i32;
    let min_z = box_.min_z.floor() as i32;
    let max_x = box_.max_x.floor() as i32;
    let max_y = box_.max_y.floor() as i32;
    let max_z = box_.max_z.floor() as i32;
    for y in min_y..=max_y {
        for z in min_z..=max_z {
            for x in min_x..=max_x {
                let id = world.block_at(x, y, z);
                if id == AIR {
                    continue;
                }
                let Some(name) = block_states::state_name(id) else {
                    continue;
                };
                if !block_states::has_collision(name) {
                    continue;
                }
                out.push(Aabb::new(
                    x as f32,
                    y as f32,
                    z as f32,
                    x as f32 + 1.0,
                    y as f32 + 1.0,
                    z as f32 + 1.0,
                ));
            }
        }
    }
    out
}

/// `Fallable` cells: whether a falling block may settle in `(x, y, z)`.
pub fn can_falling_block_settle(world: &crate::world_state::WorldState, x: i32, y: i32, z: i32) -> bool {
    if !(crate::world_state::WORLD_MIN_Y..crate::world_state::WORLD_MAX_Y).contains(&y) {
        return false;
    }
    let id = world.block_at(x, y, z);
    if id == AIR {
        return true;
    }
    block_states::state_name(id).is_some_and(physics::is_replaceable)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn item_bounding_box_is_centred_and_quarter_sized() {
        let entity = Entity::item(
            1,
            2,
            Vector3f::new(4.5, 5.0, 4.5),
            ItemStack::new(1, 1),
            Vector3f::new(0.0, 0.0, 0.0),
            "",
        );
        let bb = entity.bounding_box();
        assert_eq!(bb.min_x, 4.375);
        assert_eq!(bb.max_y, 5.25);
        assert_eq!(entity.size().eye_height(), 0.225);
        assert_eq!(entity.offset_position().y, 5.125);
    }

    #[test]
    fn falling_block_metadata_carries_the_variant() {
        let entity = Entity::falling_block(1, 2, Vector3f::new(0.5, 4.0, 0.5), 42);
        let metadata = entity.metadata();
        assert!(metadata
            .iter()
            .any(|entry| entry.key == 2 && entry.value == MetadataValue::Int(42)));
        assert_eq!(entity.network_type_id(), "minecraft:falling_block");
        assert_eq!(entity.offset_position().y, 4.49);
    }

    #[test]
    fn y_offset_stops_an_aabb_above_a_block() {
        let block = Aabb::new(0.0, 0.0, 0.0, 1.0, 1.0, 1.0);
        let moving = Aabb::new(0.25, 1.5, 0.25, 0.75, 1.75, 0.75);
        // Falling 2 blocks hits the top face at y=1.
        assert_eq!(block.calculate_y_offset(&moving, -2.0), -0.5);
        // Rising into the block's bottom face is clamped too.
        assert_eq!(
            block.calculate_y_offset(&Aabb::new(0.25, -0.5, 0.25, 0.75, -0.25, 0.75), 2.0),
            0.25
        );
    }

    #[test]
    fn block_loot_maps_blocks_to_their_item() {
        let stone = bedrock_world::block_states::network_id("minecraft:stone").unwrap();
        let item = block_item(stone).expect("stone drops itself");
        assert_eq!(item.network_id, 1);
        // Bedrock with no item form (or an unknown block) drops nothing.
        let air = block_item(AIR);
        assert!(air.is_none());
    }

    #[test]
    fn merging_combines_stacks_and_closes_the_source() {
        let mut a = Entity::item(
            1,
            1,
            Vector3f::new(0.0, 0.0, 0.0),
            ItemStack {
                network_id: 1,
                count: 3,
                ..ItemStack::empty()
            },
            Vector3f::new(0.0, 0.0, 0.0),
            "",
        );
        let mut b = Entity::item(
            2,
            2,
            Vector3f::new(0.0, 0.0, 0.0),
            ItemStack {
                network_id: 1,
                count: 5,
                ..ItemStack::empty()
            },
            Vector3f::new(0.0, 0.0, 0.0),
            "",
        );
        try_merge(&mut a, &mut b);
        assert!(a.closed);
        assert_eq!(
            match b.kind {
                EntityKind::Item { item, .. } => item.count,
                _ => 0,
            },
            8
        );
    }
}
