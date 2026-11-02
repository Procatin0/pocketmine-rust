use async_trait::async_trait;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::description::PluginDescription;
use crate::error::PluginError;
use crate::plugin::Plugin;

/// A native Rust plugin executing in the server process.
pub struct NativePluginWrapper {
    description: PluginDescription,
    enabled: AtomicBool,
}

impl NativePluginWrapper {
    pub fn new(description: PluginDescription) -> Self {
        Self {
            description,
            enabled: AtomicBool::new(false),
        }
    }
}

#[async_trait]
impl Plugin for NativePluginWrapper {
    fn description(&self) -> &PluginDescription {
        &self.description
    }

    fn is_enabled(&self) -> bool {
        self.enabled.load(Ordering::SeqCst)
    }

    async fn on_enable(&mut self) -> Result<(), PluginError> {
        self.enabled.store(true, Ordering::SeqCst);
        Ok(())
    }

    async fn on_disable(&mut self) -> Result<(), PluginError> {
        self.enabled.store(false, Ordering::SeqCst);
        Ok(())
    }
}
