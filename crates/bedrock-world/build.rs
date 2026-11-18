//! Builds the vendored cubiomes C sources (MIT, see vendor/cubiomes/LICENSE) that
//! the world generator uses for biome selection.

use std::path::Path;

fn main() {
    let dir = Path::new("vendor/cubiomes");
    let sources = [
        "biomenoise.c",
        "biomes.c",
        "generator.c",
        "layers.c",
        "noise.c",
        "util.c",
        "quadbase.c",
        "finders.c",
        "pm_shim.c",
    ];

    let mut build = cc::Build::new();
    build.include(dir).warnings(false).opt_level(2);
    for src in sources {
        build.file(dir.join(src));
    }
    build.compile("cubiomes");

    println!("cargo:rerun-if-changed={}", dir.display());
    println!("cargo:rerun-if-changed=build.rs");
}
