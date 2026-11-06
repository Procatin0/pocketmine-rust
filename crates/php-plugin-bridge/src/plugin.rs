use crate::description::PluginDescription;
use crate::error::PluginError;
use async_trait::async_trait;

#[async_trait]
pub trait Plugin: Send + Sync {
    fn description(&self) -> &PluginDescription;

    fn name(&self) -> &str {
        &self.description().name
    }

    fn is_enabled(&self) -> bool;

    /// Commands registered by this plugin (usually populated after `on_enable`).
    fn commands(&self) -> Vec<String> {
        Vec::new()
    }

    /// Event names this plugin currently listens to.
    fn listener_events(&self) -> Vec<String> {
        Vec::new()
    }

    async fn on_load(&mut self) -> Result<(), PluginError> {
        Ok(())
    }

    async fn on_enable(&mut self) -> Result<(), PluginError> {
        Ok(())
    }

    async fn on_disable(&mut self) -> Result<(), PluginError> {
        Ok(())
    }
}
