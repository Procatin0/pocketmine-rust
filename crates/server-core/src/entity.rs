//! Entity model translated from PocketMine-MP / axolotl-pm (`src/entity/`).
//!
//! Ported classes and behaviour:
//!
//! * `Entity.php` -- runtime ids, AABB, `move()` (per-axis sweep against block
//!   collision boxes), `tryChangeMovement()` (drag then gravity),
//!   `checkGroundState()`, `updateFallState()`, `entityBaseTick()` and the
//!   metadata every actor carries.
//! * `Living.php` -- health, max health, invulnerability ticks, `attack()`,
//!   `onDeath()` (loot + death animation) and knockback.
//! * `object/ItemEntity.php` and `object/FallingBlock.php` -- already present.
//! * `object/PrimedTNT.php` -- fuse and explosion.
//! * `object/ExperienceOrb.php` -- value, bob and despawn timer.
//! * `entity/projectile/*.php` -- `Projectile`, `Throwable`, `Arrow`, `Egg`,
//!   `Snowball`, `EnderPearl`, `ExperienceBottle`, `SplashPotion`, `Trident`,
//!   `IceBomb`: gravity/drag, block hit (stuck or despawn), entity hit and
//!   damage derived from speed.
//! * `EntityFactory` -- the network type table lives in [`crate::entity_types`].
//!
//! PocketMine ships no vanilla mob AI (`Mob` only applies gravity); this port
//! keeps that as the baseline and adds a deterministic wander pass so mobs are
//! not frozen statues. Everything else is a one-to-one translation.

use bedrock_protocol::packets::set_actor_data::{MetadataEntry, MetadataValue};
use bedrock_protocol::types::Vector3f;
use bedrock_protocol::ItemStack;
use bedrock_world::{block_id::AIR, block_states};

use crate::entity_types::{self, EntityType};
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
/// `EntityMetadataFlags::CRITICAL` (arrows in flight).
pub const FLAG_CRITICAL: i64 = 1 << 13;
/// `EntityMetadataFlags::IGNITED` (primed TNT).
pub const FLAG_IGNITED: i64 = 1 << 10;
/// `EntityMetadataFlags::CAN_SHOW_NAMETAG` / `ALWAYS_SHOW_NAMETAG`.
pub const FLAG_CAN_SHOW_NAMETAG: i64 = 1 << 14;
pub const FLAG_ALWAYS_SHOW_NAMETAG: i64 = 1 << 15;
/// `Living::MAX_DEAD_TICKS`: how long the death animation plays before the
/// entity is removed from the world.
pub const MAX_DEAD_TICKS: i32 = 20;
/// `Living::noDamageTicks` after a hit (half a second at 20 TPS).
pub const MOB_INVULNERABILITY_TICKS: i32 = 10;
/// Horizontal speed of the wander pass this port adds on top of PM `Mob`.
pub const MOB_WALK_SPEED: f32 = 0.09;
/// `minecraft:behavior.look_at_player`: how far a mob notices a player.
pub const MOB_LOOK_DISTANCE: f32 = 8.0;
/// Probability per tick of starting to look at a noticed player.
pub const MOB_LOOK_CHANCE: f32 = 0.02;
/// Ticks a mob holds its gaze before the head returns to the body.
pub const MOB_LOOK_TICKS: i32 = 40;
/// `ExperienceOrb::DESPAWN_TICKS` (5 minutes).
pub const XP_ORB_DESPAWN_TICKS: i32 = 6000;
/// `PrimedTNT`'s default fuse (`TAG_FUSE`, 80 ticks = 4 s).
pub const TNT_DEFAULT_FUSE: i32 = 80;
/// `PrimedTNT::explode()` explosion radius.
pub const TNT_EXPLOSION_RADIUS: f32 = 4.0;

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

/// The throwable classes of `src/entity/projectile/`. Every one is a
/// `Throwable` except `Arrow` and `Trident`, which use their own constants.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectileKind {
    Arrow,
    Snowball,
    Egg,
    EnderPearl,
    ExperienceBottle,
    SplashPotion,
    Trident,
    IceBomb,
}

impl ProjectileKind {
    /// Inverse of [`Self::network_type_id`], for `entity.spawn_projectile`.
    pub fn from_network_type_id(type_id: &str) -> Option<Self> {
        match type_id {
            "minecraft:arrow" => Some(Self::Arrow),
            "minecraft:snowball" => Some(Self::Snowball),
            "minecraft:egg" => Some(Self::Egg),
            "minecraft:ender_pearl" => Some(Self::EnderPearl),
            "minecraft:xp_bottle" => Some(Self::ExperienceBottle),
            "minecraft:splash_potion" => Some(Self::SplashPotion),
            "minecraft:thrown_trident" => Some(Self::Trident),
            "minecraft:ice_bomb" => Some(Self::IceBomb),
            _ => None,
        }
    }

    /// `Projectile::getNetworkTypeId()` of the corresponding PHP class.
    pub fn network_type_id(self) -> &'static str {
        match self {
            Self::Arrow => "minecraft:arrow",
            Self::Snowball => "minecraft:snowball",
            Self::Egg => "minecraft:egg",
            Self::EnderPearl => "minecraft:ender_pearl",
            Self::ExperienceBottle => "minecraft:xp_bottle",
            Self::SplashPotion => "minecraft:splash_potion",
            Self::Trident => "minecraft:thrown_trident",
            Self::IceBomb => "minecraft:ice_bomb",
        }
    }

    /// `Projectile::despawnsOnEntityHit()`: every projectile, arrows included,
    /// is consumed by an entity hit. Only a **block** hit leaves an arrow stuck
    /// (`Arrow::onHitBlock`); see [`Entity::projectile_tick`].
    pub fn despawns_on_entity_hit(self) -> bool {
        true
    }
}

/// `Projectile` state (`src/entity/projectile/Projectile.php`).
#[derive(Debug, Clone, PartialEq)]
pub struct Projectile {
    pub kind: ProjectileKind,
    /// `Projectile::getOwningEntity()`: the shooter's name (empty when none).
    pub owner: String,
    /// `Entity::getOwningEntityId()`: the shooter's runtime id, used to skip
    /// self-collisions during the first 5 ticks.
    pub owner_runtime_id: u64,
    /// `Projectile::$damage`, the base damage before the speed factor.
    pub damage: f32,
    /// `Arrow::$critical`.
    pub critical: bool,
    /// `Projectile::$blockHit.isNotNull()`: a stuck arrow, no longer moving.
    pub stuck: bool,
    /// `Arrow::$pickupMode` (0 none, 1 any, 2 creative only).
    pub pickup_mode: i32,
}

/// Deterministic wander state added on top of PM `Mob` (which has no AI).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MobAi {
    /// Ticks until the next direction is chosen.
    pub timer: i32,
    /// Current heading in radians.
    pub direction: f32,
    /// Ticks left holding the gaze on a player (`look_at_player`).
    pub look_ticks: i32,
    /// Head yaw (degrees) held while looking.
    pub look_yaw: f32,
}

impl Default for MobAi {
    fn default() -> Self {
        Self {
            timer: 0,
            direction: 0.0,
            look_ticks: 0,
            look_yaw: 0.0,
        }
    }
}

/// `Living` state (`src/entity/Living.php`).
#[derive(Debug, Clone, PartialEq)]
pub struct Mob {
    /// `static::getNetworkTypeId()`.
    pub type_id: &'static str,
    /// `Entity::getHealth()`.
    pub health: f32,
    /// `Living::getMaxHealth()`.
    pub max_health: f32,
    /// `Living::$deadTicks`: ticks since death, up to `MAX_DEAD_TICKS`.
    pub dead_ticks: i32,
    /// `Living::$noDamageTicks`: invulnerability after the last hit.
    pub hurt_ticks: i32,
    /// `Living::onDeath()` already dropped loot and XP.
    pub loot_dropped: bool,
    /// This port's wander pass.
    pub ai: MobAi,
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
    Projectile(Projectile),
    /// `PrimedTNT`.
    PrimedTnt {
        /// `PrimedTNT::$fuse`, ticks until the explosion.
        fuse: i32,
        /// `PrimedTNT::$worksUnderwater` (metadata `VARIANT`).
        works_underwater: bool,
    },
    /// `ExperienceOrb`.
    ExperienceOrb {
        /// `ExperienceOrb::$value` (metadata `EXPERIENCE_VALUE`).
        value: i32,
    },
    /// Any `Living` / `Mob` subclass.
    Mob(Mob),
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
    /// `Entity::getNameTag()`; empty means no name tag. `/summon` and the
    /// metadata flags `CAN_SHOW_NAMETAG`/`ALWAYS_SHOW_NAMETAG` drive it.
    pub name_tag: String,
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
            name_tag: String::new(),
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
            name_tag: String::new(),
            closed: false,
        }
    }

    /// `new Projectile(...)` / the `Arrow`, `Snowball`, `Egg`, ... classes.
    pub fn projectile(
        unique_id: i64,
        runtime_id: u64,
        kind: ProjectileKind,
        position: Vector3f,
        motion: Vector3f,
        owner: impl Into<String>,
        owner_runtime_id: u64,
        damage: f32,
        critical: bool,
    ) -> Self {
        Self {
            unique_id,
            runtime_id,
            kind: EntityKind::Projectile(Projectile {
                kind,
                owner: owner.into(),
                owner_runtime_id,
                damage,
                critical,
                stuck: false,
                pickup_mode: 1,
            }),
            position,
            motion,
            yaw: 0.0,
            pitch: 0.0,
            head_yaw: 0.0,
            on_ground: false,
            fall_distance: 0.0,
            landed_fall_distance: 0.0,
            ticks_lived: 0,
            name_tag: String::new(),
            closed: false,
        }
    }

    /// `new <MobClass>(Location)` for any type in [`crate::entity_types`].
    pub fn mob(
        unique_id: i64,
        runtime_id: u64,
        type_id: &str,
        position: Vector3f,
    ) -> Option<Self> {
        let entity_type = entity_types::lookup(type_id)?;
        if !entity_type.is_mob {
            return None;
        }
        Some(Self {
            unique_id,
            runtime_id,
            kind: EntityKind::Mob(Mob {
                type_id: entity_type.id,
                health: entity_type.max_health,
                max_health: entity_type.max_health,
                dead_ticks: 0,
                hurt_ticks: 0,
                loot_dropped: false,
                ai: MobAi::default(),
            }),
            position,
            motion: Vector3f::new(0.0, 0.0, 0.0),
            yaw: 0.0,
            pitch: 0.0,
            head_yaw: 0.0,
            on_ground: false,
            fall_distance: 0.0,
            landed_fall_distance: 0.0,
            ticks_lived: 0,
            name_tag: String::new(),
            closed: false,
        })
    }

    /// `new PrimedTNT(Location)` with `PrimedTNT::$fuse`.
    pub fn primed_tnt(
        unique_id: i64,
        runtime_id: u64,
        position: Vector3f,
        fuse: i32,
    ) -> Self {
        Self {
            unique_id,
            runtime_id,
            kind: EntityKind::PrimedTnt {
                fuse,
                works_underwater: false,
            },
            position,
            motion: Vector3f::new(0.0, 0.0, 0.0),
            yaw: 0.0,
            pitch: 0.0,
            head_yaw: 0.0,
            on_ground: false,
            fall_distance: 0.0,
            landed_fall_distance: 0.0,
            ticks_lived: 0,
            name_tag: String::new(),
            closed: false,
        }
    }

    /// `new ExperienceOrb(Location, $value)`.
    pub fn experience_orb(
        unique_id: i64,
        runtime_id: u64,
        position: Vector3f,
        value: i32,
    ) -> Self {
        Self {
            unique_id,
            runtime_id,
            kind: EntityKind::ExperienceOrb { value: value.max(1) },
            position,
            motion: Vector3f::new(0.0, 0.0, 0.0),
            yaw: 0.0,
            pitch: 0.0,
            head_yaw: 0.0,
            on_ground: false,
            fall_distance: 0.0,
            landed_fall_distance: 0.0,
            ticks_lived: 0,
            name_tag: String::new(),
            closed: false,
        }
    }

    /// `getInitialSizeInfo()`: the type table entry, or the item/falling-block
    /// defaults when the type is unknown.
    pub fn size(&self) -> EntitySize {
        match &self.kind {
            EntityKind::Item { .. } => EntitySize::new(0.25, 0.25),
            EntityKind::FallingBlock { .. } => EntitySize::new(0.98, 0.98),
            EntityKind::Projectile(projectile) => entity_types::lookup(
                projectile.kind.network_type_id(),
            )
            .map(|t| EntitySize::new(t.height, t.width))
            .unwrap_or_else(|| EntitySize::new(0.25, 0.25)),
            EntityKind::PrimedTnt { .. } => EntitySize::new(0.98, 0.98),
            EntityKind::ExperienceOrb { .. } => EntitySize::new(0.25, 0.25),
            EntityKind::Mob(mob) => entity_types::lookup(mob.type_id)
                .map(|t| EntitySize::new(t.height, t.width))
                .unwrap_or_else(|| EntitySize::new(1.8, 0.6)),
        }
    }

    /// The registry entry for this entity, when it has one.
    pub fn entity_type(&self) -> Option<&'static EntityType> {
        match &self.kind {
            EntityKind::Projectile(projectile) => {
                entity_types::lookup(projectile.kind.network_type_id())
            }
            EntityKind::Mob(mob) => entity_types::lookup(mob.type_id),
            _ => entity_types::lookup(self.network_type_id()),
        }
    }

    /// `getNetworkTypeId()`.
    pub fn network_type_id(&self) -> &'static str {
        match &self.kind {
            EntityKind::Item { .. } => "minecraft:item",
            EntityKind::FallingBlock { .. } => "minecraft:falling_block",
            EntityKind::Projectile(projectile) => projectile.kind.network_type_id(),
            EntityKind::PrimedTnt { .. } => "minecraft:tnt",
            EntityKind::ExperienceOrb { .. } => "minecraft:xp_orb",
            EntityKind::Mob(mob) => mob.type_id,
        }
    }

    /// `getInitialGravity()`.
    pub fn gravity(&self) -> f32 {
        self.entity_type().map(|t| t.gravity).unwrap_or(0.04)
    }

    /// `getInitialDragMultiplier()`.
    pub fn drag(&self) -> f32 {
        self.entity_type().map(|t| t.drag).unwrap_or(0.02)
    }

    /// `applyDragBeforeGravity()`: items and projectiles drag first.
    pub fn drag_before_gravity(&self) -> bool {
        matches!(self.kind, EntityKind::Item { .. }) || self.entity_type().is_some_and(|t| t.drag_before_gravity)
    }

    pub fn bounding_box(&self) -> Aabb {
        Aabb::from_position(self.position, self.size())
    }

    /// `Entity::getOffsetPosition()`: items render 0.125 up, falling blocks
    /// and TNT 0.49 up (their box is centred in the block cell).
    pub fn offset_position(&self) -> Vector3f {
        let dy = match self.kind {
            EntityKind::Item { .. } => 0.125,
            EntityKind::FallingBlock { .. } => 0.49,
            EntityKind::PrimedTnt { .. } => 0.49,
            _ => 0.0,
        };
        Vector3f::new(self.position.x, self.position.y + dy, self.position.z)
    }

    /// `Entity::syncNetworkData()` plus the per-class properties.
    pub fn metadata(&self) -> Vec<MetadataEntry> {
        let size = self.size();
        let mut flags = FLAG_HAS_COLLISION | FLAG_AFFECTED_BY_GRAVITY;
        if let EntityKind::Projectile(projectile) = &self.kind {
            if projectile.critical {
                flags |= FLAG_CRITICAL;
            }
        }
        if matches!(self.kind, EntityKind::PrimedTnt { .. }) {
            flags |= FLAG_IGNITED;
        }
        if !self.name_tag.is_empty() {
            // `Entity::syncNetworkData()`: a name tag implies the two visibility
            // flags, otherwise the client never renders it.
            flags |= FLAG_CAN_SHOW_NAMETAG | FLAG_ALWAYS_SHOW_NAMETAG;
        }
        let mut metadata = vec![
            MetadataEntry::new(0, MetadataValue::Long(flags)),
            MetadataEntry::new(3, MetadataValue::Byte(0)),
            MetadataEntry::new(4, MetadataValue::String(self.name_tag.clone())),
            MetadataEntry::new(5, MetadataValue::Long(-1)),
            MetadataEntry::new(6, MetadataValue::Long(0)),
            MetadataEntry::new(37, MetadataValue::Long(-1)),
            MetadataEntry::new(38, MetadataValue::Float(1.0)),
            MetadataEntry::new(53, MetadataValue::Float(size.width)),
            MetadataEntry::new(54, MetadataValue::Float(size.height)),
        ];
        match &self.kind {
            EntityKind::FallingBlock { block_id } => metadata.push(MetadataEntry::new(
                2,
                MetadataValue::Int(*block_id as i32),
            )),
            EntityKind::Projectile(projectile) => {
                if projectile.owner_runtime_id != 0 {
                    // `EntityMetadataProperties::SHOOTER_ID`.
                    metadata.push(MetadataEntry::new(
                        17,
                        MetadataValue::Long(projectile.owner_runtime_id as i64),
                    ));
                }
            }
            EntityKind::PrimedTnt {
                fuse,
                works_underwater,
            } => {
                // `FUSE_LENGTH` and `VARIANT` (`PrimedTNT::syncNetworkData`).
                metadata.push(MetadataEntry::new(55, MetadataValue::Int(*fuse)));
                metadata.push(MetadataEntry::new(
                    2,
                    MetadataValue::Int(i32::from(*works_underwater)),
                ));
            }
            EntityKind::ExperienceOrb { value } => metadata.push(MetadataEntry::new(
                15,
                MetadataValue::Int(*value),
            )),
            _ => {}
        }
        metadata
    }

    /// `Entity::onUpdate()` for one tick: movement, then `entityBaseTick`.
    /// Returns whether the sweep hit a block (used by projectiles).
    pub fn on_update(&mut self, world: &crate::world_state::WorldState) -> bool {
        if self.closed {
            return false;
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
        let mut collided = false;
        if self.motion.x != 0.0 || self.motion.y != 0.0 || self.motion.z != 0.0 {
            let (dx, dy, dz) = (self.motion.x, self.motion.y, self.motion.z);
            collided = self.move_entity(world, dx, dy, dz);
        }

        self.entity_base_tick(world);
        collided
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
    ) -> bool {
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

        wanted_x != dx || wanted_y != dy || wanted_z != dz
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

    /// `Entity::isAlive()` for living entities; non-living types are "alive"
    /// until they close.
    pub fn is_alive(&self) -> bool {
        match &self.kind {
            EntityKind::Mob(mob) => mob.health > 0.0,
            _ => !self.closed,
        }
    }

    pub fn is_mob(&self) -> bool {
        matches!(self.kind, EntityKind::Mob(_))
    }

    pub fn is_projectile(&self) -> bool {
        matches!(self.kind, EntityKind::Projectile(_))
    }

    /// `Entity::getHealth()`; `None` for non-living entities.
    pub fn health(&self) -> Option<f32> {
        match &self.kind {
            EntityKind::Mob(mob) => Some(mob.health),
            _ => None,
        }
    }

    pub fn max_health(&self) -> Option<f32> {
        match &self.kind {
            EntityKind::Mob(mob) => Some(mob.max_health),
            _ => None,
        }
    }

    /// `Living::attack()`: applies `amount` unless the invulnerability window
    /// is still open, and returns whether the hit killed the entity.
    pub fn damage(&mut self, amount: f32) -> DamageOutcome {
        let EntityKind::Mob(mob) = &mut self.kind else {
            return DamageOutcome::Ignored;
        };
        if amount <= 0.0 || mob.health <= 0.0 || mob.hurt_ticks > 0 {
            return DamageOutcome::Ignored;
        }
        mob.hurt_ticks = MOB_INVULNERABILITY_TICKS;
        mob.health = (mob.health - amount).max(0.0);
        if mob.health <= 0.0 {
            mob.dead_ticks = 0;
            DamageOutcome::Killed
        } else {
            DamageOutcome::Damaged
        }
    }

    /// `Living::knockBack($x, $z, $force)` with the default vertical lift.
    pub fn knock_back(&mut self, dx: f32, dz: f32, force: f32) {
        let length = (dx * dx + dz * dz).sqrt();
        if length <= 0.0 {
            return;
        }
        let scale = force / length;
        self.motion.x = self.motion.x / 2.0 + dx * scale;
        self.motion.z = self.motion.z / 2.0 + dz * scale;
        self.motion.y = (self.motion.y / 2.0 + force).min(0.4);
        self.on_ground = false;
    }

    /// `Living::onUpdate()` after the movement pass: death animation, hurt
    /// countdown and the wander pass this port adds on top of PM's AI-less
    /// `Mob`.
    pub fn mob_tick(&mut self, collided: bool) {
        let dead_ticks = match &mut self.kind {
            EntityKind::Mob(mob) if mob.health <= 0.0 => {
                mob.dead_ticks += 1;
                Some(mob.dead_ticks)
            }
            EntityKind::Mob(mob) => {
                if mob.hurt_ticks > 0 {
                    mob.hurt_ticks -= 1;
                }
                None
            }
            _ => return,
        };
        if let Some(ticks) = dead_ticks {
            if ticks >= MAX_DEAD_TICKS {
                self.closed = true;
            }
            return;
        }

        let (mut timer, mut direction, hurt) = match &self.kind {
            EntityKind::Mob(mob) => (mob.ai.timer, mob.ai.direction, mob.hurt_ticks),
            _ => return,
        };
        timer -= 1;
        if timer <= 0 {
            let seed = self.unique_id as u64
                ^ (self.ticks_lived as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15);
            direction = pseudo_random(seed) * std::f32::consts::TAU;
            timer = 20 + (pseudo_random(seed ^ 0x51ED_270B) * 60.0) as i32;
        }

        // A hurt mob keeps the knockback the attacker applied.
        if hurt == 0 && self.on_ground {
            // `direction` is a Bedrock yaw (radians): `Entity::getDirection-
            // Vector()` is `(-sin yaw, cos yaw)`, so the body faces the way it
            // walks. Using `(cos, sin)` made every mob crab-walk 90 degrees
            // off its spawn facing while its body kept the old heading.
            self.motion.x = -direction.sin() * MOB_WALK_SPEED;
            self.motion.z = direction.cos() * MOB_WALK_SPEED;
            // Walked into a wall (`move_entity` zeroed the horizontal motion):
            // jump, like a vanilla land mob. A pure vertical collision (the
            // landing itself) must not trigger a jump.
            if collided && self.motion.x == 0.0 && self.motion.z == 0.0 {
                self.motion.y = 0.42;
                self.on_ground = false;
            }
            self.yaw = direction.to_degrees().rem_euclid(360.0);
            self.head_yaw = self.yaw;
        }

        if let EntityKind::Mob(mob) = &mut self.kind {
            mob.ai.timer = timer;
            mob.ai.direction = direction;
        }
    }

    /// `minecraft:behavior.look_at_player`, simplified: a healthy mob within
    /// [`MOB_LOOK_DISTANCE`] of a player has a low per-tick chance of turning
    /// its **head** (not the body) towards the nearest one, holding the gaze
    /// for [`MOB_LOOK_TICKS`] and then returning the head to the body yaw.
    /// Returns whether the head yaw changed this tick.
    pub fn mob_look_tick(&mut self, players: &[(f32, f32, f32)]) -> bool {
        let EntityKind::Mob(mob) = &mut self.kind else {
            return false;
        };
        // Death animation and knockback keep the current head.
        if mob.health <= 0.0 || mob.hurt_ticks > 0 {
            return false;
        }
        let previous = self.head_yaw;
        if mob.ai.look_ticks > 0 {
            mob.ai.look_ticks -= 1;
            self.head_yaw = if mob.ai.look_ticks == 0 {
                self.yaw
            } else {
                mob.ai.look_yaw
            };
            return self.head_yaw != previous;
        }

        let nearest = players
            .iter()
            .filter_map(|(x, y, z)| {
                let (dx, dy, dz) = (*x - self.position.x, *y - self.position.y, *z - self.position.z);
                let distance = dx * dx + dy * dy + dz * dz;
                (distance <= MOB_LOOK_DISTANCE * MOB_LOOK_DISTANCE).then_some((distance, dx, dz))
            })
            .min_by(|a, b| a.0.total_cmp(&b.0));
        let Some((_, dx, dz)) = nearest else {
            return false;
        };
        let seed = self.unique_id as u64
            ^ (self.ticks_lived as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15)
            ^ 0x10_0C_A7;
        if pseudo_random(seed) >= MOB_LOOK_CHANCE {
            return false;
        }
        // Bedrock yaw: the direction vector is `(-sin yaw, cos yaw)`.
        let yaw = (-dx).atan2(dz).to_degrees().rem_euclid(360.0);
        mob.ai.look_yaw = yaw;
        mob.ai.look_ticks = MOB_LOOK_TICKS;
        self.head_yaw = yaw;
        self.head_yaw != previous
    }

    /// `Projectile::move()` result handling: aim at the velocity, then stick to
    /// a block (arrow) or despawn (every other projectile).
    pub fn projectile_tick(&mut self, collided: bool) {
        let EntityKind::Projectile(projectile) = &mut self.kind else {
            return;
        };
        if projectile.stuck {
            return;
        }
        let horizontal = (self.motion.x * self.motion.x + self.motion.z * self.motion.z).sqrt();
        if horizontal > MOTION_THRESHOLD {
            self.yaw = self.motion.x.atan2(self.motion.z).to_degrees();
            self.pitch = self.motion.y.atan2(horizontal).to_degrees();
            self.head_yaw = self.yaw;
        }
        if collided {
            if projectile.kind != ProjectileKind::Arrow {
                self.closed = true;
            } else {
                // `Projectile::$blockHit` + `Arrow::onHit`: a stuck arrow is
                // no longer critical and stops simulating movement.
                projectile.stuck = true;
                projectile.critical = false;
                self.motion = Vector3f::new(0.0, 0.0, 0.0);
                self.on_ground = true;
            }
        }
    }

    /// `Projectile::$blockHit.isNotNull()`.
    pub fn projectile_stuck(&self) -> bool {
        matches!(&self.kind, EntityKind::Projectile(projectile) if projectile.stuck)
    }

    /// `Arrow::getPickupMode()` (0 none, 1 any, 2 creative only).
    pub fn projectile_pickup_mode(&self) -> i32 {
        match &self.kind {
            EntityKind::Projectile(projectile) => projectile.pickup_mode,
            _ => 0,
        }
    }

    /// `Projectile::despawnsOnEntityHit()`.
    pub fn projectile_despawns_on_hit(&self) -> bool {
        match &self.kind {
            EntityKind::Projectile(projectile) => projectile.kind.despawns_on_entity_hit(),
            _ => false,
        }
    }

    /// `Entity::getOwningEntityId()` of the shooter (0 when none).
    pub fn projectile_owner_runtime_id(&self) -> u64 {
        match &self.kind {
            EntityKind::Projectile(projectile) => projectile.owner_runtime_id,
            _ => 0,
        }
    }

    /// `Projectile::getResultDamage()` (speed-scaled) for the current tick.
    pub fn projectile_damage(&self) -> f32 {
        let EntityKind::Projectile(projectile) = &self.kind else {
            return 0.0;
        };
        if projectile.stuck {
            return 0.0;
        }
        let speed = (self.motion.x * self.motion.x
            + self.motion.y * self.motion.y
            + self.motion.z * self.motion.z)
            .sqrt();
        (speed * projectile.damage).ceil()
    }

    /// `Living::getDrops()` for the mob table: `(item name, min, max)`.
    pub fn mob_loot(&self) -> Vec<(&'static str, u32, u32)> {
        let EntityKind::Mob(mob) = &self.kind else {
            return Vec::new();
        };
        match mob.type_id.strip_prefix("minecraft:").unwrap_or(mob.type_id) {
            "chicken" => vec![("minecraft:chicken", 1, 1), ("minecraft:feather", 0, 2)],
            "cow" => vec![("minecraft:leather", 0, 2), ("minecraft:beef", 1, 3)],
            "mooshroom" => vec![("minecraft:leather", 0, 2), ("minecraft:beef", 1, 3)],
            "pig" => vec![("minecraft:porkchop", 1, 3)],
            "sheep" => vec![("minecraft:mutton", 1, 2), ("minecraft:wool", 1, 1)],
            "rabbit" => vec![("minecraft:rabbit", 0, 1), ("minecraft:rabbit_hide", 0, 1)],
            "zombie" | "husk" | "zombie_villager" => vec![("minecraft:rotten_flesh", 0, 2)],
            "zombie_pigman" => vec![(
                "minecraft:rotten_flesh",
                0,
                2,
            )],
            "skeleton" | "stray" | "wither_skeleton" => vec![
                ("minecraft:bone", 0, 2),
                ("minecraft:arrow", 0, 2),
            ],
            "creeper" => vec![("minecraft:gunpowder", 0, 2)],
            "spider" | "cave_spider" => vec![("minecraft:string", 0, 2)],
            "enderman" => vec![("minecraft:ender_pearl", 0, 1)],
            "blaze" => vec![("minecraft:blaze_rod", 0, 1)],
            "slime" | "magma_cube" => vec![("minecraft:slime_ball", 0, 2)],
            "squid" => vec![("minecraft:ink_sac", 1, 3)],
            "witch" => vec![("minecraft:glowstone_dust", 0, 2)],
            "guardian" | "elder_guardian" => vec![(
                "minecraft:prismarine_shard",
                0,
                2,
            )],
            _ => Vec::new(),
        }
    }

    /// `Living::getXpDropAmount()`.
    pub fn mob_xp(&self) -> i32 {
        match &self.kind {
            EntityKind::Mob(mob) => entity_types::lookup(mob.type_id)
                .map(|t| t.xp)
                .unwrap_or(0),
            _ => 0,
        }
    }
}

/// Result of [`Entity::damage`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DamageOutcome {
    /// Already dead, invulnerable or not a living entity.
    Ignored,
    /// Took the hit and survived.
    Damaged,
    /// Took the hit and died (`Living::onDeath()` runs in the world tick).
    Killed,
}

/// Deterministic `lcg_value()` stand-in: `[0, 1)` from a 64-bit seed.
/// PocketMine uses `mt_rand`/`lcg_value`; the port must stay deterministic
/// because entity loot and wander runs inside the world tick.
pub fn pseudo_random(seed: u64) -> f32 {
    let mut x = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
    x ^= x >> 33;
    x = x.wrapping_mul(0xff51afd7ed558ccd);
    x ^= x >> 33;
    ((x >> 40) as f32) / (1u32 << 24) as f32
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

    #[test]
    fn mobs_use_the_registry_size_and_health() {
        let zombie = Entity::mob(1, 2, "zombie", Vector3f::new(0.5, 64.0, 0.5)).unwrap();
        assert_eq!(zombie.network_type_id(), "minecraft:zombie");
        assert_eq!(zombie.health(), Some(20.0));
        assert_eq!(zombie.size(), EntitySize::new(1.9, 0.6));
        assert!(zombie.is_mob());
        // A non-living id is not spawnable as a mob.
        assert!(Entity::mob(1, 2, "minecraft:item", Vector3f::new(0.0, 0.0, 0.0)).is_none());
    }

    #[test]
    fn damage_respects_invulnerability_and_kills() {
        let mut mob = Entity::mob(1, 2, "cow", Vector3f::new(0.0, 64.0, 0.0)).unwrap();
        assert_eq!(mob.damage(3.0), DamageOutcome::Damaged);
        assert_eq!(mob.health(), Some(7.0));
        // `Living::$noDamageTicks` blocks a second hit in the same window.
        assert_eq!(mob.damage(3.0), DamageOutcome::Ignored);
        if let EntityKind::Mob(state) = &mut mob.kind {
            state.hurt_ticks = 0;
        }
        assert_eq!(mob.damage(50.0), DamageOutcome::Killed);
        assert!(!mob.is_alive());
        // The death animation holds the entity for MAX_DEAD_TICKS.
        for _ in 0..MAX_DEAD_TICKS {
            mob.mob_tick(false);
        }
        assert!(mob.closed);
    }

    #[test]
    fn arrow_speed_scales_damage_and_sticks_to_blocks() {
        let mut arrow = Entity::projectile(
            1,
            2,
            ProjectileKind::Arrow,
            Vector3f::new(0.5, 64.0, 0.5),
            Vector3f::new(0.0, 0.0, 3.0),
            "Steve",
            7,
            2.0,
            true,
        );
        assert_eq!(arrow.projectile_damage(), 6.0);
        arrow.projectile_tick(true);
        assert!(matches!(
            &arrow.kind,
            EntityKind::Projectile(p) if p.stuck && !p.critical
        ));
        // A stuck arrow deals no more damage and does not despawn.
        assert_eq!(arrow.projectile_damage(), 0.0);
        assert!(!arrow.closed);
    }

    #[test]
    fn throwables_despawn_on_impact() {
        let mut egg = Entity::projectile(
            1,
            2,
            ProjectileKind::Egg,
            Vector3f::new(0.5, 64.0, 0.5),
            Vector3f::new(0.0, 0.0, 1.0),
            "Steve",
            7,
            0.0,
            false,
        );
        egg.projectile_tick(true);
        assert!(egg.closed);
    }

    #[test]
    fn tnt_metadata_carries_the_fuse_and_ignited_flag() {
        let tnt = Entity::primed_tnt(1, 2, Vector3f::new(0.5, 64.0, 0.5), 40);
        let metadata = tnt.metadata();
        assert!(metadata
            .iter()
            .any(|entry| entry.key == 55 && entry.value == MetadataValue::Int(40)));
        let flags = metadata
            .iter()
            .find(|entry| entry.key == 0)
            .map(|entry| match entry.value {
                MetadataValue::Long(value) => value,
                _ => 0,
            })
            .unwrap();
        assert_eq!(flags & FLAG_IGNITED, FLAG_IGNITED);
    }

    #[test]
    fn mob_loot_is_translated_from_the_php_drop_tables() {
        let zombie = Entity::mob(1, 2, "zombie", Vector3f::new(0.0, 64.0, 0.0)).unwrap();
        assert_eq!(zombie.mob_loot(), vec![("minecraft:rotten_flesh", 0, 2)]);
        assert_eq!(zombie.mob_xp(), 5);
        let villager = Entity::mob(1, 2, "villager", Vector3f::new(0.0, 64.0, 0.0)).unwrap();
        assert!(villager.mob_loot().is_empty());
    }
}
