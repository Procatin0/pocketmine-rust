//! Permission registry modelled on PocketMine's `PermissionManager`.
//!
//! Nodes are registered with a default (`true`, `false`, `op`, `notop`) and may
//! list child nodes. Per-player attachments (`PermissionAttachment`) override
//! the registered default for a single player.

use std::collections::HashMap;

use parking_lot::RwLock;

/// `Permission::getDefault()`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PermissionDefault {
    True,
    False,
    Op,
    NotOp,
}

impl PermissionDefault {
    fn resolve(self, is_op: bool, allow_cheats: bool) -> bool {
        match self {
            Self::True => true,
            Self::False => false,
            Self::Op => is_op || allow_cheats,
            Self::NotOp => !is_op,
        }
    }
}

/// A registered permission node.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Permission {
    pub name: String,
    pub default: PermissionDefault,
    pub children: Vec<String>,
}

/// The nodes PocketMine's defaults rely on for the commands this port has.
pub const COMMAND_HELP: &str = "pocketmine.command.help";
pub const COMMAND_TELEPORT_SELF: &str = "pocketmine.command.teleport.self";
pub const COMMAND_TELEPORT_OTHER: &str = "pocketmine.command.teleport.other";
pub const COMMAND_GAMEMODE_SELF: &str = "pocketmine.command.gamemode.self";
pub const COMMAND_GAMEMODE_OTHER: &str = "pocketmine.command.gamemode.other";
pub const COMMAND_SETWORLDSPAWN: &str = "pocketmine.command.setworldspawn";
pub const COMMAND_SAY: &str = "pocketmine.command.say";
pub const COMMAND_ME: &str = "pocketmine.command.me";
pub const COMMAND_TELL: &str = "pocketmine.command.tell";
pub const COMMAND_LIST: &str = "pocketmine.command.list";
pub const COMMAND_KICK: &str = "pocketmine.command.kick";
pub const COMMAND_GIVE_SELF: &str = "pocketmine.command.give.self";
pub const COMMAND_GIVE_OTHER: &str = "pocketmine.command.give.other";
pub const COMMAND_CLEAR_SELF: &str = "pocketmine.command.clear.self";
pub const COMMAND_CLEAR_OTHER: &str = "pocketmine.command.clear.other";
pub const COMMAND_KILL_SELF: &str = "pocketmine.command.kill.self";
pub const COMMAND_KILL_OTHER: &str = "pocketmine.command.kill.other";
pub const COMMAND_SPAWNPOINT_SELF: &str = "pocketmine.command.spawnpoint.self";
pub const COMMAND_SPAWNPOINT_OTHER: &str = "pocketmine.command.spawnpoint.other";
pub const COMMAND_DEFAULTGAMEMODE: &str = "pocketmine.command.defaultgamemode";
pub const COMMAND_DIFFICULTY: &str = "pocketmine.command.difficulty";
pub const COMMAND_SEED: &str = "pocketmine.command.seed";
pub const COMMAND_TIME_SET: &str = "pocketmine.command.time.set";
pub const COMMAND_TIME_ADD: &str = "pocketmine.command.time.add";
pub const COMMAND_TIME_QUERY: &str = "pocketmine.command.time.query";
pub const COMMAND_OP_GIVE: &str = "pocketmine.command.op.give";
pub const COMMAND_OP_TAKE: &str = "pocketmine.command.op.take";
pub const COMMAND_VERSION: &str = "pocketmine.command.version";
pub const COMMAND_BAN_PLAYER: &str = "pocketmine.command.ban.player";
pub const COMMAND_BAN_IP: &str = "pocketmine.command.ban.ip";
pub const COMMAND_BAN_LIST: &str = "pocketmine.command.ban.list";
pub const COMMAND_UNBAN_PLAYER: &str = "pocketmine.command.unban.player";
pub const COMMAND_UNBAN_IP: &str = "pocketmine.command.unban.ip";
pub const COMMAND_WHITELIST_ADD: &str = "pocketmine.command.whitelist.add";
pub const COMMAND_WHITELIST_REMOVE: &str = "pocketmine.command.whitelist.remove";
pub const COMMAND_WHITELIST_ENABLE: &str = "pocketmine.command.whitelist.enable";
pub const COMMAND_WHITELIST_DISABLE: &str = "pocketmine.command.whitelist.disable";
pub const COMMAND_WHITELIST_LIST: &str = "pocketmine.command.whitelist.list";
pub const COMMAND_WHITELIST_RELOAD: &str = "pocketmine.command.whitelist.reload";
pub const COMMAND_STOP: &str = "pocketmine.command.stop";
pub const COMMAND_PLUGINS: &str = "pocketmine.command.plugins";
pub const COMMAND_SAVE_PERFORM: &str = "pocketmine.command.save.perform";
pub const COMMAND_SAVE_ENABLE: &str = "pocketmine.command.save.enable";
pub const COMMAND_SAVE_DISABLE: &str = "pocketmine.command.save.disable";
pub const COMMAND_TPS: &str = "pocketmine.command.tps";
pub const BROADCAST_ADMIN: &str = "pocketmine.broadcast.admin";

/// Thread-safe registry plus per-player attachments.
#[derive(Debug)]
pub struct PermissionManager {
    permissions: RwLock<HashMap<String, Permission>>,
    attachments: RwLock<HashMap<String, HashMap<String, bool>>>,
}

impl PartialEq for PermissionManager {
    fn eq(&self, other: &Self) -> bool {
        *self.permissions.read() == *other.permissions.read()
            && *self.attachments.read() == *other.attachments.read()
    }
}

impl Default for PermissionManager {
    fn default() -> Self {
        Self::new()
    }
}

impl PermissionManager {
    /// Registry seeded with PocketMine's `DefaultPermissionNames` defaults.
    pub fn new() -> Self {
        let manager = Self {
            permissions: RwLock::new(HashMap::new()),
            attachments: RwLock::new(HashMap::new()),
        };
        manager.register_defaults();
        manager
    }

    pub fn register(&self, name: &str, default: PermissionDefault, children: &[&str]) {
        self.permissions.write().insert(
            name.to_string(),
            Permission {
                name: name.to_string(),
                default,
                children: children.iter().map(|child| child.to_string()).collect(),
            },
        );
    }

    fn register_defaults(&self) {
        use PermissionDefault::{False, NotOp, Op, True};

        self.register(COMMAND_HELP, True, &[]);
        self.register(COMMAND_TELEPORT_SELF, Op, &[]);
        self.register(COMMAND_TELEPORT_OTHER, Op, &[]);
        self.register(COMMAND_GAMEMODE_SELF, Op, &[]);
        self.register(COMMAND_GAMEMODE_OTHER, Op, &[]);
        self.register(COMMAND_SETWORLDSPAWN, Op, &[]);
        self.register(COMMAND_SAY, Op, &[]);
        self.register(COMMAND_ME, True, &[]);
        self.register(COMMAND_TELL, True, &[]);
        self.register(COMMAND_LIST, True, &[]);
        self.register(COMMAND_KICK, Op, &[]);
        self.register(COMMAND_GIVE_SELF, Op, &[]);
        self.register(COMMAND_GIVE_OTHER, Op, &[]);
        self.register(COMMAND_CLEAR_SELF, True, &[]);
        self.register(COMMAND_CLEAR_OTHER, Op, &[]);
        self.register(COMMAND_KILL_SELF, True, &[]);
        self.register(COMMAND_KILL_OTHER, Op, &[]);
        self.register(COMMAND_SPAWNPOINT_SELF, Op, &[]);
        self.register(COMMAND_SPAWNPOINT_OTHER, Op, &[]);
        self.register(COMMAND_DEFAULTGAMEMODE, Op, &[]);
        self.register(COMMAND_DIFFICULTY, Op, &[]);
        self.register(COMMAND_SEED, Op, &[]);
        self.register(COMMAND_TIME_SET, Op, &[]);
        self.register(COMMAND_TIME_ADD, Op, &[]);
        self.register(COMMAND_TIME_QUERY, True, &[]);
        self.register(COMMAND_OP_GIVE, Op, &[]);
        self.register(COMMAND_OP_TAKE, Op, &[]);
        self.register(COMMAND_VERSION, True, &[]);
        self.register(COMMAND_BAN_PLAYER, Op, &[]);
        self.register(COMMAND_BAN_IP, Op, &[]);
        self.register(COMMAND_BAN_LIST, Op, &[]);
        self.register(COMMAND_UNBAN_PLAYER, Op, &[]);
        self.register(COMMAND_UNBAN_IP, Op, &[]);
        self.register(COMMAND_WHITELIST_ADD, Op, &[]);
        self.register(COMMAND_WHITELIST_REMOVE, Op, &[]);
        self.register(COMMAND_WHITELIST_ENABLE, Op, &[]);
        self.register(COMMAND_WHITELIST_DISABLE, Op, &[]);
        self.register(COMMAND_WHITELIST_LIST, Op, &[]);
        self.register(COMMAND_WHITELIST_RELOAD, Op, &[]);
        self.register(COMMAND_STOP, Op, &[]);
        self.register(COMMAND_PLUGINS, Op, &[]);
        self.register(COMMAND_SAVE_PERFORM, Op, &[]);
        self.register(COMMAND_SAVE_ENABLE, Op, &[]);
        self.register(COMMAND_SAVE_DISABLE, Op, &[]);
        self.register(COMMAND_TPS, Op, &[]);
        self.register(BROADCAST_ADMIN, Op, &[]);

        // Unused defaults kept for parity with DefaultPermissionNames.
        self.register("pocketmine.broadcast.user", True, &[]);
        self.register("pocketmine.command.time.start", Op, &[]);
        self.register("pocketmine.command.time.stop", Op, &[]);
        self.register("pocketmine.command.effect.self", Op, &[]);
        self.register("pocketmine.command.effect.other", Op, &[]);
        self.register("pocketmine.command.enchant.self", Op, &[]);
        self.register("pocketmine.command.enchant.other", Op, &[]);
        self.register("pocketmine.command.xp.self", Op, &[]);
        self.register("pocketmine.command.xp.other", Op, &[]);
        self.register("pocketmine.command.title.self", Op, &[]);
        self.register("pocketmine.command.title.other", Op, &[]);
        self.register("pocketmine.command.particle", Op, &[]);
        self.register("pocketmine.command.status", Op, &[]);
        self.register("pocketmine.command.gc", Op, &[]);
        self.register("pocketmine.command.timings", Op, &[]);
        self.register("pocketmine.command.dumpmemory", Op, &[]);
        self.register("pocketmine.command.transferserver", Op, &[]);
        let _ = (False, NotOp);
    }

    pub fn default_of(&self, node: &str) -> Option<PermissionDefault> {
        self.permissions.read().get(node).map(|permission| permission.default)
    }

    pub fn children_of(&self, node: &str) -> Vec<String> {
        self.permissions
            .read()
            .get(node)
            .map(|permission| permission.children.clone())
            .unwrap_or_default()
    }

    /// Grants or denies `node` for `player` (`PermissionAttachment`).
    pub fn set_permission(&self, player: &str, node: &str, value: bool) {
        self.attachments
            .write()
            .entry(player.to_lowercase())
            .or_default()
            .insert(node.to_string(), value);
    }

    /// Removes an attachment entry; returns whether one existed.
    pub fn unset_permission(&self, player: &str, node: &str) -> bool {
        let mut attachments = self.attachments.write();
        let Some(entries) = attachments.get_mut(&player.to_lowercase()) else {
            return false;
        };
        entries.remove(node).is_some()
    }

    /// Removes every attachment of a player, like dropping the attachment.
    pub fn clear_permissions(&self, player: &str) {
        self.attachments.write().remove(&player.to_lowercase());
    }

    /// `PermissionManager::hasPermission()`: attachment, then children, then the
    /// registered default. Unregistered nodes fall back to the operator state.
    pub fn has(&self, player: &str, is_op: bool, allow_cheats: bool, node: &str) -> bool {
        self.has_with_depth(player, is_op, allow_cheats, node, 0)
    }

    fn has_with_depth(
        &self,
        player: &str,
        is_op: bool,
        allow_cheats: bool,
        node: &str,
        depth: usize,
    ) -> bool {
        if depth > 8 {
            return false;
        }
        if let Some(value) = self
            .attachments
            .read()
            .get(&player.to_lowercase())
            .and_then(|entries| entries.get(node))
        {
            return *value;
        }
        let permission = self.permissions.read().get(node).cloned();
        let Some(permission) = permission else {
            return is_op || allow_cheats;
        };
        for child in &permission.children {
            if self.has_with_depth(player, is_op, allow_cheats, child, depth + 1) {
                return true;
            }
        }
        permission.default.resolve(is_op, allow_cheats)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_pocketmine() {
        let permissions = PermissionManager::new();

        // Everyone may use these.
        assert!(permissions.has("Guest", false, false, COMMAND_HELP));
        assert!(permissions.has("Guest", false, false, COMMAND_ME));
        assert!(permissions.has("Guest", false, false, COMMAND_CLEAR_SELF));
        assert!(permissions.has("Guest", false, false, COMMAND_KILL_SELF));

        // Operator-only nodes.
        assert!(!permissions.has("Guest", false, false, COMMAND_STOP));
        assert!(!permissions.has("Guest", false, false, COMMAND_BAN_PLAYER));
        assert!(permissions.has("Admin", true, false, COMMAND_STOP));

        // `allow-cheats` lifts op defaults for everybody.
        assert!(permissions.has("Guest", false, true, COMMAND_STOP));
    }

    #[test]
    fn attachments_override_defaults() {
        let permissions = PermissionManager::new();
        permissions.set_permission("Guest", COMMAND_STOP, true);
        assert!(permissions.has("Guest", false, false, COMMAND_STOP));

        permissions.set_permission("Admin", COMMAND_STOP, false);
        assert!(!permissions.has("Admin", true, false, COMMAND_STOP));

        assert!(permissions.unset_permission("Admin", COMMAND_STOP));
        assert!(permissions.has("Admin", true, false, COMMAND_STOP));
    }

    #[test]
    fn children_grant_their_parent() {
        let permissions = PermissionManager::new();
        permissions.register(
            "example.parent",
            PermissionDefault::False,
            &["example.child"],
        );
        permissions.register("example.child", PermissionDefault::True, &[]);

        assert!(permissions.has("Guest", false, false, "example.parent"));
        assert_eq!(
            permissions.children_of("example.parent"),
            vec!["example.child".to_string()]
        );
    }
}
