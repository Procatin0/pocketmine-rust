use async_trait::async_trait;
use crate::description::PluginDescription;
use crate::error::PluginError;

#[async_trait]
pub trait Plugin: Send + Sync {
    fn description(&self) -> &PluginDescription;

    fn name(&self) -> &str {
        &self.description().name
    }

    fn is_enabled(&self) -> bool;

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
