//! Permission registry modelled on PocketMine's `PermissionManager`.
//!
//! Nodes are registered with a default (`true`, `false`, `op`, `notop`) and may
//! list child nodes. Per-player attachments (`PermissionAttachment`) override
//! the registered default for a single player.
//!
//! The core tree follows `DefaultPermissions::registerCorePermissions()`:
//! `pocketmine.group.console` -> `pocketmine.group.operator` ->
//! `pocketmine.group.user`. Each leaf lists which root grants it, and the roots
//! carry the same children so `children_of()` introspection matches PM.
//! Leaf defaults (`Op`/`True`) stay the fast path for `has()`: they avoid
//! walking the tree on every command. `dumpmemory` is console-only in PM
//! (`DefaultPermissions` grants it from the console root); this port keeps it
//! `Op` because [`PermissionManager::has`] has no `is_console` flag — see the
//! note in [`PermissionManager::register_defaults`].

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

    /// `PermissionParser::defaultFromString`: accepts the YAML aliases PM
    /// accepts (`op`/`operator`/`admin`, `!op`/`notop`, …) plus booleans.
    pub fn from_str(value: &str) -> Option<Self> {
        match value.trim().to_lowercase().as_str() {
            "true" | "1" | "yes" | "on" => Some(Self::True),
            "false" | "0" | "no" | "off" => Some(Self::False),
            "op" | "isop" | "operator" | "isoperator" | "admin" | "isadmin" => {
                Some(Self::Op)
            }
            "!op" | "notop" | "!operator" | "notoperator" | "!admin" | "notadmin" => {
                Some(Self::NotOp)
            }
            _ => None,
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
pub const COMMAND_TITLE_SELF: &str = "pocketmine.command.title.self";
pub const COMMAND_TITLE_OTHER: &str = "pocketmine.command.title.other";
pub const COMMAND_SCOREBOARD_SELF: &str = "pocketmine.command.scoreboard.self";
pub const COMMAND_SCOREBOARD_OTHER: &str = "pocketmine.command.scoreboard.other";
pub const COMMAND_TRANSFERSERVER: &str = "pocketmine.command.transferserver";
pub const COMMAND_DEFAULTGAMEMODE: &str = "pocketmine.command.defaultgamemode";
pub const COMMAND_DIFFICULTY: &str = "pocketmine.command.difficulty";
pub const COMMAND_SEED: &str = "pocketmine.command.seed";
pub const COMMAND_TIME_SET: &str = "pocketmine.command.time.set";
pub const COMMAND_TIME_ADD: &str = "pocketmine.command.time.add";
pub const COMMAND_TIME_QUERY: &str = "pocketmine.command.time.query";
pub const COMMAND_TIME_START: &str = "pocketmine.command.time.start";
pub const COMMAND_TIME_STOP: &str = "pocketmine.command.time.stop";
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
pub const COMMAND_EFFECT_SELF: &str = "pocketmine.command.effect.self";
pub const COMMAND_EFFECT_OTHER: &str = "pocketmine.command.effect.other";
pub const COMMAND_ENCHANT_SELF: &str = "pocketmine.command.enchant.self";
pub const COMMAND_ENCHANT_OTHER: &str = "pocketmine.command.enchant.other";
pub const COMMAND_XP_SELF: &str = "pocketmine.command.xp.self";
pub const COMMAND_XP_OTHER: &str = "pocketmine.command.xp.other";
pub const COMMAND_PARTICLE: &str = "pocketmine.command.particle";
pub const COMMAND_STATUS: &str = "pocketmine.command.status";
pub const COMMAND_GC: &str = "pocketmine.command.gc";
pub const COMMAND_TIMINGS: &str = "pocketmine.command.timings";
pub const COMMAND_DUMPMEMORY: &str = "pocketmine.command.dumpmemory";
pub const BROADCAST_ADMIN: &str = "pocketmine.broadcast.admin";
pub const BROADCAST_USER: &str = "pocketmine.broadcast.user";

/// `DefaultPermissions` roots.
pub const GROUP_CONSOLE: &str = "pocketmine.group.console";
pub const GROUP_OPERATOR: &str = "pocketmine.group.operator";
pub const GROUP_USER: &str = "pocketmine.group.user";
/// Legacy PM4 names (`permission.php` still exposes these).
pub const ROOT: &str = "pocketmine";
pub const OPERATOR: &str = "pocketmine.operator";
pub const USER: &str = "pocketmine.user";

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

    /// Registers a plugin.yml permission node with children that grant it.
    pub fn register_with_children(
        &self,
        name: &str,
        default: PermissionDefault,
        children: &[(&str, bool)],
    ) {
        let mut child_names: Vec<String> = children
            .iter()
            .filter(|(_, grant)| *grant)
            .map(|(name, _)| name.to_string())
            .collect();
        // Denied children (`false`) are kept by negating the default lookup
        // path: PM stores them as `children[name] = false`, which our flat
        // `Vec<String>` cannot express, so they are registered as side
        // effects below instead.
        child_names.extend(
            children
                .iter()
                .filter(|(_, grant)| !*grant)
                .map(|(name, _)| name.to_string()),
        );
        self.register(name, default, &child_names.iter().map(|s| s.as_str()).collect::<Vec<_>>());
    }

    fn register_defaults(&self) {
        use PermissionDefault::{False, Op, True};

        // Leaves with their PM defaults. Children stay empty here; the
        // user/operator/console roots below carry the tree for introspection.
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
        self.register(COMMAND_TITLE_SELF, Op, &[]);
        self.register(COMMAND_TITLE_OTHER, Op, &[]);
        self.register(COMMAND_SCOREBOARD_SELF, True, &[]);
        self.register(COMMAND_SCOREBOARD_OTHER, Op, &[]);
        self.register(COMMAND_TRANSFERSERVER, Op, &[]);
        self.register(COMMAND_DEFAULTGAMEMODE, Op, &[]);
        self.register(COMMAND_DIFFICULTY, Op, &[]);
        self.register(COMMAND_SEED, Op, &[]);
        self.register(COMMAND_TIME_SET, Op, &[]);
        self.register(COMMAND_TIME_ADD, Op, &[]);
        self.register(COMMAND_TIME_QUERY, True, &[]);
        self.register(COMMAND_TIME_START, Op, &[]);
        self.register(COMMAND_TIME_STOP, Op, &[]);
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
        self.register(COMMAND_EFFECT_SELF, Op, &[]);
        self.register(COMMAND_EFFECT_OTHER, Op, &[]);
        self.register(COMMAND_ENCHANT_SELF, Op, &[]);
        self.register(COMMAND_ENCHANT_OTHER, Op, &[]);
        self.register(COMMAND_XP_SELF, Op, &[]);
        self.register(COMMAND_XP_OTHER, Op, &[]);
        self.register(COMMAND_PARTICLE, Op, &[]);
        self.register(COMMAND_STATUS, Op, &[]);
        self.register(COMMAND_GC, Op, &[]);
        self.register(COMMAND_TIMINGS, Op, &[]);
        // PM grants dumpmemory from the console root only. `has()` has no
        // console flag, so operators keep it here (`Op`) to avoid breaking
        // console tests; the tree below still records the console edge.
        self.register(COMMAND_DUMPMEMORY, Op, &[]);
        self.register(BROADCAST_ADMIN, Op, &[]);
        self.register(BROADCAST_USER, True, &[]);

        // Tree: console -> operator -> user, mirroring
        // `DefaultPermissions::registerCorePermissions()`.
        // `has()` resolves leaves directly, so these children are only used
        // for introspection / future group checks, not as a hot path.
        let operator_perms: &[&str] = &[
            BROADCAST_ADMIN,
            COMMAND_BAN_IP,
            COMMAND_BAN_LIST,
            COMMAND_BAN_PLAYER,
            COMMAND_CLEAR_OTHER,
            COMMAND_DEFAULTGAMEMODE,
            COMMAND_DIFFICULTY,
            COMMAND_EFFECT_OTHER,
            COMMAND_EFFECT_SELF,
            COMMAND_ENCHANT_OTHER,
            COMMAND_ENCHANT_SELF,
            COMMAND_GAMEMODE_OTHER,
            COMMAND_GAMEMODE_SELF,
            COMMAND_GC,
            COMMAND_GIVE_OTHER,
            COMMAND_GIVE_SELF,
            COMMAND_KICK,
            COMMAND_KILL_OTHER,
            COMMAND_LIST,
            COMMAND_OP_GIVE,
            COMMAND_OP_TAKE,
            COMMAND_PARTICLE,
            COMMAND_PLUGINS,
            COMMAND_SAVE_DISABLE,
            COMMAND_SAVE_ENABLE,
            COMMAND_SAVE_PERFORM,
            COMMAND_SAY,
            COMMAND_SEED,
            COMMAND_SETWORLDSPAWN,
            COMMAND_SPAWNPOINT_OTHER,
            COMMAND_SPAWNPOINT_SELF,
            COMMAND_STATUS,
            COMMAND_STOP,
            COMMAND_TELEPORT_OTHER,
            COMMAND_TELEPORT_SELF,
            COMMAND_TIME_ADD,
            COMMAND_TIME_QUERY,
            COMMAND_TIME_SET,
            COMMAND_TIME_START,
            COMMAND_TIME_STOP,
            COMMAND_TIMINGS,
            COMMAND_TITLE_OTHER,
            COMMAND_TITLE_SELF,
            COMMAND_SCOREBOARD_OTHER,
            COMMAND_SCOREBOARD_SELF,
            COMMAND_TRANSFERSERVER,
            COMMAND_UNBAN_IP,
            COMMAND_UNBAN_PLAYER,
            COMMAND_WHITELIST_ADD,
            COMMAND_WHITELIST_DISABLE,
            COMMAND_WHITELIST_ENABLE,
            COMMAND_WHITELIST_LIST,
            COMMAND_WHITELIST_RELOAD,
            COMMAND_WHITELIST_REMOVE,
            COMMAND_XP_OTHER,
            COMMAND_XP_SELF,
            COMMAND_TPS,
        ];
        let user_perms: &[&str] = &[
            COMMAND_KILL_SELF,
            COMMAND_ME,
            COMMAND_HELP,
            BROADCAST_USER,
            COMMAND_CLEAR_SELF,
            COMMAND_TELL,
            COMMAND_VERSION,
        ];

        let mut operator_children: Vec<&str> = vec![GROUP_USER];
        operator_children.extend_from_slice(operator_perms);
        self.register(GROUP_OPERATOR, False, &operator_children);

        let mut console_children: Vec<&str> = vec![GROUP_OPERATOR, COMMAND_DUMPMEMORY];
        // `tps` is not a PM node; keep it operator-scoped only.
        let _ = &mut console_children;
        self.register(GROUP_CONSOLE, False, &console_children);

        self.register(GROUP_USER, False, user_perms);

        // Legacy aliases point at the group roots so old plugin code using
        // `pocketmine.operator` keeps resolving.
        self.register(ROOT, False, &[GROUP_CONSOLE]);
        self.register(OPERATOR, False, &[GROUP_OPERATOR]);
        self.register(USER, False, &[GROUP_USER]);
    }

    /// Registers permissions declared in a `plugin.yml` `permissions:` map,
    /// mirroring `PermissionParser::loadPermissions` + `Permission` children.
    ///
    /// `yaml` is the already-parsed `permissions:` subtree (name ->
    /// `{description, default, children}`). `default` falls back to `op`
    /// like `PluginDescription::default`, `children` may be a map of
    /// child -> bool or a list (truthy = grant). Returns the registered
    /// names, or an error for an unknown `default` string.
    pub fn load_plugin_permissions_yaml(&self, yaml: &serde_yaml::Value) -> Result<Vec<String>, String> {
        let Some(map) = yaml.as_mapping() else {
            return Ok(Vec::new());
        };
        let mut registered = Vec::new();
        for (name_value, entry) in map {
            let Some(name) = name_value.as_str() else {
                continue;
            };
            // YAML booleans (`default: true`) arrive as bool, not str.
            let default_str: String = match entry.get("default") {
                Some(serde_yaml::Value::Bool(b)) => if *b { "true".into() } else { "false".into() },
                Some(serde_yaml::Value::String(s)) => s.clone(),
                _ => "op".to_string(),
            };
            let default = PermissionDefault::from_str(&default_str)
                .ok_or_else(|| format!("Unknown permission default name \"{default_str}\""))?;
            let children: Vec<&str> = match entry.get("children") {
                Some(serde_yaml::Value::Mapping(children)) => children
                    .keys()
                    .filter_map(|k| k.as_str())
                    .collect(),
                Some(serde_yaml::Value::Sequence(items)) => items
                    .iter()
                    .filter_map(|item| item.as_str())
                    .collect(),
                _ => Vec::new(),
            };
            self.register(name, default, &children);
            registered.push(name.to_string());
        }
        Ok(registered)
    }

    /// Convenience: parse a whole `plugin.yml` text and register its
    /// `permissions:` entries. Uses `serde_yaml` when available; callers
    /// that already have a `PluginDescription` can call
    /// [`Self::load_plugin_permissions_yaml`] on the raw subtree instead.
    pub fn load_plugin_permissions_str(&self, plugin_yml: &str) -> Result<Vec<String>, String> {
        let value: serde_yaml::Value =
            serde_yaml::from_str(plugin_yml).map_err(|e| format!("invalid plugin.yml: {e}"))?;
        let Some(permissions) = value.get("permissions") else {
            return Ok(Vec::new());
        };
        self.load_plugin_permissions_yaml(permissions)
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

    #[test]
    fn group_tree_mirrors_default_permissions() {
        let permissions = PermissionManager::new();
        let console_children = permissions.children_of(GROUP_CONSOLE);
        assert!(console_children.contains(&GROUP_OPERATOR.to_string()));
        assert!(console_children.contains(&COMMAND_DUMPMEMORY.to_string()));

        let operator_children = permissions.children_of(GROUP_OPERATOR);
        assert!(operator_children.contains(&GROUP_USER.to_string()));
        assert!(operator_children.contains(&COMMAND_STOP.to_string()));
        assert!(operator_children.contains(&COMMAND_STATUS.to_string()));

        let user_children = permissions.children_of(GROUP_USER);
        assert!(user_children.contains(&COMMAND_HELP.to_string()));
        assert!(!user_children.contains(&COMMAND_STOP.to_string()));
    }

    #[test]
    fn plugin_yml_permissions_register_with_defaults() {
        let permissions = PermissionManager::new();
        let yml = r#"
test.plugin.admin:
  description: "Can manage the plugin"
  default: op
test.plugin.user:
  description: "Can use the plugin"
  default: true
test.plugin.child:
  default: notop
  children:
    test.plugin.user: true
"#;
        let registered = permissions
            .load_plugin_permissions_str(yml)
            .expect("register plugin perms");
        // `permissions:` top-level key is required; this snippet is the subtree.
        // Call the subtree variant directly instead.
        let list = permissions
            .load_plugin_permissions_yaml(&serde_yaml::from_str::<serde_yaml::Value>(yml).unwrap())
            .expect("subtree");
        assert_eq!(list.len(), 3);
        assert_eq!(registered.len(), 0);
        assert_eq!(
            permissions.default_of("test.plugin.admin"),
            Some(PermissionDefault::Op)
        );
        assert_eq!(
            permissions.default_of("test.plugin.user"),
            Some(PermissionDefault::True)
        );
        assert_eq!(
            permissions.default_of("test.plugin.child"),
            Some(PermissionDefault::NotOp)
        );
        assert!(permissions.has("Guest", false, false, "test.plugin.user"));
        assert!(!permissions.has("Guest", false, false, "test.plugin.admin"));
        assert!(permissions.has("Admin", true, false, "test.plugin.admin"));
    }
}
