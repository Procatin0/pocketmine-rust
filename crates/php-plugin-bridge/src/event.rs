use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

use crate::context::PluginAction;
use crate::error::PluginError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[repr(u8)]
pub enum EventPriority {
    Lowest = 0,
    Low = 1,
    Normal = 2,
    High = 3,
    Highest = 4,
    Monitor = 5,
}

impl Default for EventPriority {
    fn default() -> Self {
        Self::Normal
    }
}

pub trait Event: Send + Sync {
    fn name(&self) -> &str;
    fn is_cancellable(&self) -> bool {
        false
    }
    fn is_cancelled(&self) -> bool {
        false
    }
    fn set_cancelled(&mut self, _cancelled: bool) {}
    fn to_json(&self) -> serde_json::Value;
}

pub type EventCallback = Arc<dyn Fn(&mut dyn Event) -> Result<Vec<PluginAction>, PluginError> + Send + Sync>;

#[derive(Clone)]
pub struct RegisteredListener {
    pub plugin_name: String,
    pub event_name: String,
    pub priority: EventPriority,
    pub ignore_cancelled: bool,
    pub callback: EventCallback,
}

pub struct EventDispatcher {
    listeners: RwLock<HashMap<String, Vec<RegisteredListener>>>,
}

impl Default for EventDispatcher {
    fn default() -> Self {
        Self::new()
    }
}

impl EventDispatcher {
    pub fn new() -> Self {
        Self {
            listeners: RwLock::new(HashMap::new()),
        }
    }

    pub fn register_listener(&self, listener: RegisteredListener) {
        let mut map = self.listeners.write();
        let list = map.entry(listener.event_name.clone()).or_default();
        list.push(listener);
        // Sort by priority ascending (Lowest first, Monitor last)
        list.sort_by_key(|l| l.priority);
    }

    pub fn unregister_plugin_listeners(&self, plugin_name: &str) {
        let mut map = self.listeners.write();
        for list in map.values_mut() {
            list.retain(|l| l.plugin_name != plugin_name);
        }
    }

    pub fn dispatch(&self, event: &mut dyn Event) -> Result<Vec<PluginAction>, PluginError> {
        let event_name = event.name().to_string();
        let listeners = {
            let map = self.listeners.read();
            map.get(&event_name).cloned().unwrap_or_default()
        };

        let mut actions = Vec::new();
        for listener in listeners {
            if event.is_cancelled() && !listener.ignore_cancelled && listener.priority != EventPriority::Monitor {
                continue;
            }

            let result = (listener.callback)(event)?;
            actions.extend(result);
        }

        Ok(actions)
    }

    pub fn listener_count(&self, event_name: &str) -> usize {
        let map = self.listeners.read();
        map.get(event_name).map(|l| l.len()).unwrap_or(0)
    }
}
