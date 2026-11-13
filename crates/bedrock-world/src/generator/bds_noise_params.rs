//! Bedrock Edition Noise Parameters
//!
//! This module contains the noise configuration for Bedrock Edition's 1.18+ terrain generation.
//! 
//! **IMPORTANT**: The seed values below are PLACEHOLDERS (Java Edition values).
//! Bedrock Edition uses DIFFERENT seeds for the 6 climate noises.
//! 
//! TODO: Replace with actual Bedrock Edition seeds when discovered via reverse engineering.
//! 
//! Sources for discovery:
//! - BDS binary analysis (bedrock_server.exe)
//! - Community documentation (wiki.vg, bedrock.dev, MCBE modding community)
//! - Comparison with Java Edition seeds (known to differ)

/// Climate noise indices (matching Java/Bedrock)
pub const CLIMATE_TEMPERATURE: usize = 0;
pub const CLIMATE_HUMIDITY: usize = 1;
pub const CLIMATE_CONTINENTALNESS: usize = 2;
pub const CLIMATE_EROSION: usize = 3;
pub const CLIMATE_PEAKS_AND_VALLEYS: usize = 4;
pub const CLIMATE_WEIRDNESS: usize = 5;

/// Bedrock Edition climate noise seeds
/// 
/// Each noise uses a pair of u64 seeds (low, high) for Xoroshiro128+ initialization.
/// These are the JAVA EDITION values - Bedrock uses DIFFERENT ones!
/// 
/// Java Edition seeds (from niduy/minecraft-world-generator, verified):
/// - Temperature:      (0x5c7e6b29735f0d7f, 0xf7d86f1bbc734988)
/// - Humidity:         (0x81bb4d22e8dc168e, 0xf1c8b4bea16303cd)
/// - Continentalness:  (0x83886c9d0ae3a662, 0xafa638a61b42e8ad)
/// - Erosion:          (0xd02491e6058f6fd8, 0x4792512c94c17a80)
/// - Peaks & Valleys:  (0x080518cf6af25384, 0x3f3dfb40a54febd5)
/// - Weirdness:        (0xefc8ef4d36102b34, 0x1beeeb324a0f24ea)
///
/// Bedrock Edition seeds: UNKNOWN - Need to extract from BDS binary
/// 
/// Research notes:
/// - Bedrock 1.18+ uses same 6-noise climate system as Java
/// - Strings "continentalness", "erosion", "weirdness", "temperature", "humidity" found in BDS binary
/// - Noise seeds are compile-time constants in the binary
/// - Can be extracted via: Ghidra/IDA disassembly, or runtime memory inspection
pub const BEDROCK_CLIMATE_SEEDS: [(u64, u64); 6] = [
    // TEMPERATURE - TODO: Replace with Bedrock value
    (0x5c7e6b29735f0d7f, 0xf7d86f1bbc734988),
    // HUMIDITY - TODO: Replace with Bedrock value  
    (0x81bb4d22e8dc168e, 0xf1c8b4bea16303cd),
    // CONTINENTALNESS - TODO: Replace with Bedrock value
    (0x83886c9d0ae3a662, 0xafa638a61b42e8ad),
    // EROSION - TODO: Replace with Bedrock value
    (0xd02491e6058f6fd8, 0x4792512c94c17a80),
    // PEAKS_AND_VALLEYS - TODO: Replace with Bedrock value
    (0x080518cf6af25384, 0x3f3dfb40a54febd5),
    // WEIRDNESS - TODO: Replace with Bedrock value
    (0xefc8ef4d36102b34, 0x1beeeb324a0f24ea),
];

/// Amplitude parameters for each climate noise (same as Java for now)
/// These control the octave amplitudes for DoublePerlinNoise
pub const CLIMATE_PARAMS: [[f64; 9]; 6] = [
    [1.5, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],         // temperature
    [1.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],         // humidity
    [1.0, 1.0, 2.0, 2.0, 2.0, 1.0, 1.0, 1.0, 1.0],         // continentalness
    [1.0, 1.0, 0.0, 1.0, 1.0, 0.0, 0.0, 0.0, 0.0],         // erosion
    [1.0, 1.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],         // peaks_and_valleys
    [1.0, 2.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],         // weirdness
];

/// Minimum octave for each climate noise
pub const CLIMATE_OMIN: [i32; 6] = [-10, -8, -9, -9, -3, -7];

/// Get climate seed by name (for documentation/runtime configuration)
pub fn get_climate_seed_by_name(name: &str) -> Option<(u64, u64)> {
    let idx = match name.to_lowercase().as_str() {
        "temperature" => CLIMATE_TEMPERATURE,
        "humidity" => CLIMATE_HUMIDITY,
        "continentalness" => CLIMATE_CONTINENTALNESS,
        "erosion" => CLIMATE_EROSION,
        "peaks" | "peaks_and_valleys" => CLIMATE_PEAKS_AND_VALLEYS,
        "weirdness" => CLIMATE_WEIRDNESS,
        _ => return None,
    };
    Some(BEDROCK_CLIMATE_SEEDS[idx])
}

/// Set climate seed by name (for testing/custom seeds)
/// This modifies the global seeds - use with caution!
pub fn set_climate_seed(name: &str, low: u64, high: u64) -> bool {
    // Note: This would require mutable static, use runtime config instead
    // Provided for API completeness
    let _ = (name, low, high);
    false
}

/// Bedrock-specific string seed handling
/// 
/// Bedrock uses MurmurHash3 for string->seed conversion (unlike Java's hashCode)
pub fn bedrock_string_seed(input: &str) -> u64 {
    crate::generator::bds_seed_hash::bedrock_string_seed_to_u64(input)
}

/// Configuration for a complete Bedrock world generation setup
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct BedrockNoiseConfig {
    pub climate_seeds: [(u64, u64); 6],
    pub climate_params: [[f64; 9]; 6],
    pub climate_omin: [i32; 6],
    pub sea_level: i32,
    pub biome_mapping: BiomeMappingConfig,
}

impl Default for BedrockNoiseConfig {
    fn default() -> Self {
        Self {
            climate_seeds: BEDROCK_CLIMATE_SEEDS,
            climate_params: CLIMATE_PARAMS,
            climate_omin: CLIMATE_OMIN,
            sea_level: 62, // Bedrock sea level
            biome_mapping: BiomeMappingConfig::default(),
        }
    }
}

impl BedrockNoiseConfig {
    /// Create config with custom climate seeds
    pub fn with_seeds(mut self, seeds: [(u64, u64); 6]) -> Self {
        self.climate_seeds = seeds;
        self
    }

    /// Create config from JSON (for external configuration)
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }

    /// Export to JSON for sharing/documenting discovered seeds
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap()
    }
}

/// Biome mapping configuration (Bedrock legacy biome IDs)
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct BiomeMappingConfig {
    /// Temperature thresholds for biome transitions
    pub temp_thresholds: [f64; 4],
    /// Humidity thresholds for biome transitions
    pub humidity_thresholds: [f64; 3],
    /// Continentalness threshold for ocean/land
    pub continentalness_ocean: f64,
    pub continentalness_coast: f64,
}

impl Default for BiomeMappingConfig {
    fn default() -> Self {
        Self {
            temp_thresholds: [0.15, 0.5, 0.7, 1.0],
            humidity_thresholds: [0.2, 0.4, 0.7],
            continentalness_ocean: 0.1,
            continentalness_coast: 0.3,
        }
    }
}

/// Known Bedrock Edition seeds for testing (when discovered)
/// 
/// Format: (seed_string, expected_biome_at_origin, expected_height_range)
/// These can be used to verify seed discovery
pub const KNOWN_BEDROCK_TEST_SEEDS: &[(&str, u8, (i32, i32))] = &[
    // ("seed_string", biome_id, (min_height, max_height))
    // Add discovered seeds here when found
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_placeholder_seeds_not_zero() {
        for (low, high) in BEDROCK_CLIMATE_SEEDS {
            assert_ne!(low, 0, "Low seed should not be zero");
            assert_ne!(high, 0, "High seed should not be zero");
            assert_ne!(low, high, "Low and high should differ");
        }
    }

    #[test]
    fn test_climate_params_lengths() {
        assert_eq!(CLIMATE_PARAMS.len(), 6);
        assert_eq!(CLIMATE_OMIN.len(), 6);
        // Continentalness has most octaves (9)
        assert_eq!(CLIMATE_PARAMS[CLIMATE_CONTINENTALNESS].len(), 9);
    }

    #[test]
    fn test_get_climate_seed_by_name() {
        assert!(get_climate_seed_by_name("temperature").is_some());
        assert!(get_climate_seed_by_name("continentalness").is_some());
        assert!(get_climate_seed_by_name("weirdness").is_some());
        assert!(get_climate_seed_by_name("invalid").is_none());
    }

    #[test]
    fn test_bedrock_string_seed() {
        // Empty string -> 0
        assert_eq!(bedrock_string_seed(""), 0);
        // Deterministic
        assert_eq!(bedrock_string_seed("test"), bedrock_string_seed("test"));
        // Different strings -> different seeds (almost always)
        assert_ne!(bedrock_string_seed("a"), bedrock_string_seed("b"));
    }

    #[test]
    fn test_default_config() {
        let config = BedrockNoiseConfig::default();
        assert_eq!(config.sea_level, 62);
        assert_eq!(config.climate_seeds.len(), 6);
    }

    #[test]
    fn test_config_serialization() {
        let config = BedrockNoiseConfig::default();
        let json = config.to_json();
        assert!(json.contains("climate_seeds"));
        assert!(json.contains("sea_level"));
        
        let parsed = BedrockNoiseConfig::from_json(&json).unwrap();
        assert_eq!(parsed.sea_level, 62);
        assert_eq!(parsed.climate_seeds.len(), 6);
    }
}