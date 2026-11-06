use serde_json::Value;

/// Synchronous bridge from the PHP plugin runtime back into the Rust server.
///
/// PHP may emit `{"type":"host_call","id":..,"method":..,"params":..}` lines at
/// any point while a request is in flight. The worker thread forwards those
/// calls to the currently installed host and writes the result back to PHP.
pub trait PluginHost: Send + Sync {
    /// Invoke `method` with `params`.
    ///
    /// Returning `Err` makes the bridged call fail with `ok:false`; the PHP
    /// side is expected to use its fallback value in that case.
    fn call(&self, method: &str, params: &Value) -> Result<Value, String>;
}
