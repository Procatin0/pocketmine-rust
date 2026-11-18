// Thin C bridge so the Rust side never needs cubiomes' internal struct layout.
#include <stdint.h>
#include <stdlib.h>

#include "generator.h"
#include "util.h"

struct Generator *pm_generator_new(int mc, uint32_t flags, uint64_t seed)
{
    struct Generator *g = (struct Generator *)malloc(sizeof(struct Generator));
    if (!g)
        return NULL;
    setupGenerator(g, mc, flags);
    applySeed(g, 0, seed);
    return g;
}

int pm_generator_biome(const struct Generator *g, int scale, int x, int y, int z)
{
    return getBiomeAt(g, scale, x, y, z);
}

void pm_generator_free(struct Generator *g)
{
    free(g);
}

// The MC version the world generator asks cubiomes for, chosen here so the Rust
// side never has to know the enum's numeric value.
int pm_mc_target(void)
{
    return MC_1_18_2;
}

// The six 1.18 climate parameters (temperature, humidity, continentalness,
// erosion, depth, weirdness), each scaled by 10000, with y = 0 so the terrain
// and the biome agree about which continent a column belongs to.
int pm_sample_climate(const struct Generator *g, int x, int z, int64_t *np)
{
    // sampleBiomeNoise returns the biome id, not a status. We only want the six
    // parameters, so the biome step is skipped and success is reported explicitly.
    sampleBiomeNoise(&g->bn, np, x, 0, z, NULL, SAMPLE_NO_BIOME);
    return 0;
}

// Names for the ids cubiomes reports, so the Rust side can check its own id
// mapping against the library instead of guessing.
const char *pm_biome_name(int mc, int id)
{
    const char *name = biome2str(mc, id);
    return name ? name : "unknown";
}
