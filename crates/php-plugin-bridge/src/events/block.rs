use serde_json::json;
use crate::context::PlayerContext;
use crate::event::Event;

/// Fired when a player breaks a block. Cancellable.
pub struct BlockBreakEvent {
    pub player: PlayerContext,
    pub world: String,
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub block_id: u32,
    pub cancelled: bool,
}

impl BlockBreakEvent {
    pub fn new(player: PlayerContext, world: impl Into<String>, x: i32, y: i32, z: i32, block_id: u32) -> Self {
        Self {
            player,
            world: world.into(),
            x,
            y,
            z,
            block_id,
            cancelled: false,
        }
    }
}

impl Event for BlockBreakEvent {
    fn name(&self) -> &str {
        "BlockBreakEvent"
    }

    fn is_cancellable(&self) -> bool {
        true
    }

    fn is_cancelled(&self) -> bool {
        self.cancelled
    }

    fn set_cancelled(&mut self, cancel: bool) {
        self.cancelled = cancel;
    }

    fn to_json(&self) -> serde_json::Value {
        json!({
            "player": self.player,
            "world": self.world,
            "x": self.x,
            "y": self.y,
            "z": self.z,
            "block_id": self.block_id,
            "cancelled": self.cancelled
        })
    }
}

/// Fired when a player places a block. Cancellable.
pub struct BlockPlaceEvent {
    pub player: PlayerContext,
    pub world: String,
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub block_id: u32,
    pub cancelled: bool,
}

impl BlockPlaceEvent {
    pub fn new(player: PlayerContext, world: impl Into<String>, x: i32, y: i32, z: i32, block_id: u32) -> Self {
        Self {
            player,
            world: world.into(),
            x,
            y,
            z,
            block_id,
            cancelled: false,
        }
    }
}

impl Event for BlockPlaceEvent {
    fn name(&self) -> &str {
        "BlockPlaceEvent"
    }

    fn is_cancellable(&self) -> bool {
        true
    }

    fn is_cancelled(&self) -> bool {
        self.cancelled
    }

    fn set_cancelled(&mut self, cancel: bool) {
        self.cancelled = cancel;
    }

    fn to_json(&self) -> serde_json::Value {
        json!({
            "player": self.player,
            "world": self.world,
            "x": self.x,
            "y": self.y,
            "z": self.z,
            "block_id": self.block_id,
            "cancelled": self.cancelled
        })
    }
}
