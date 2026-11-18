//! Biome selection through the vendored [cubiomes] library.
//!
//! cubiomes (MIT, (c) 2020 Cubitect, <https://github.com/Cubitect/cubiomes>)
//! implements Minecraft's own biome generator, so the world does not need a
//! reimplementation of the climate tree: we hand it a seed and ask which biome a
//! block column belongs to. Only the overworld surface is asked for; the terrain
//! itself is still ours.
//!
//! The C sources live in `vendor/cubiomes` and are compiled by `build.rs`;
//! `vendor/cubiomes/pm_shim.c` is the small bridge that keeps cubiomes' struct
//! layout out of Rust.
//!
//! [cubiomes]: https://github.com/Cubitect/cubiomes

use std::ffi::c_void;
use std::fmt;

/// Opaque `struct Generator` from cubiomes.
#[repr(C)]
struct GeneratorOpaque {
    _private: [u8; 0],
}

extern "C" {
    fn pm_mc_target() -> i32;
    fn pm_generator_new(mc: i32, flags: u32, seed: u64) -> *mut GeneratorOpaque;
    fn pm_generator_biome(g: *const GeneratorOpaque, scale: i32, x: i32, y: i32, z: i32) -> i32;
    fn pm_generator_free(g: *mut GeneratorOpaque);
    fn pm_sample_climate(g: *const GeneratorOpaque, x: i32, z: i32, np: *mut i64) -> i32;
    fn pm_biome_name(mc: i32, id: i32) -> *const std::os::raw::c_char;
}

/// A cubiomes overworld generator for one seed.
pub struct Cubiomes {
    generator: *mut GeneratorOpaque,
}

// cubiomes' `getBiomeAt` does not mutate the generator, so a shared handle is
// safe to use from several chunk workers at once.
unsafe impl Send for Cubiomes {}
unsafe impl Sync for Cubiomes {}

impl Cubiomes {
    /// Sets up the overworld generator for `seed`.
    pub fn new(seed: u64) -> Option<Self> {
        let generator = unsafe { pm_generator_new(pm_mc_target(), 0, seed) };
        if generator.is_null() {
            None
        } else {
            Some(Self { generator })
        }
    }

    /// The name cubiomes gives this biome id, for diagnostics.
    pub fn biome_name(id: u8) -> String {
        let mc = unsafe { pm_mc_target() };
        let ptr = unsafe { pm_biome_name(mc, id as i32) };
        if ptr.is_null() {
            return "unknown".to_string();
        }
        unsafe { std::ffi::CStr::from_ptr(ptr) }
            .to_string_lossy()
            .into_owned()
    }

    /// The six 1.18 climate parameters at `(x, z)`, in vanilla's order
    /// (temperature, humidity, continentalness, erosion, depth, weirdness) and
    /// already scaled to the -1..1 range.
    pub fn climate_at(&self, x: i32, z: i32) -> Option<[f64; 6]> {
        let mut np = [0i64; 6];
        let status = unsafe { pm_sample_climate(self.generator, x, z, np.as_mut_ptr()) };
        if status != 0 {
            return None;
        }
        let mut out = [0.0f64; 6];
        for (slot, value) in out.iter_mut().zip(np) {
            *slot = value as f64 / 10000.0;
        }
        Some(out)
    }

    /// Java biome id at the block column `(x, y, z)`, where `y` is the height the
    /// column is built to (the 1.18 generator uses it as the climate depth).
    pub fn biome_at(&self, x: i32, y: i32, z: i32) -> Option<u8> {
        let id = unsafe { pm_generator_biome(self.generator, 1, x, y, z) };
        if id < 0 || id > u8::MAX as i32 {
            None
        } else {
            Some(id as u8)
        }
    }
}

impl Drop for Cubiomes {
    fn drop(&mut self) {
        if !self.generator.is_null() {
            unsafe { pm_generator_free(self.generator) };
        }
    }
}

impl fmt::Debug for Cubiomes {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Cubiomes").finish_non_exhaustive()
    }
}

// Keeps the C `c_void` import used on every platform where the shim's return
// types are spelled out; harmless otherwise.
#[allow(dead_code)]
type Opaque = c_void;
