# Texture Encode/Decode: Performance Guidelines

## Performance

Texture encode/decode is a performance-sensitive hot path. Implementations must consider:

- Parallelism via rayon for block-level operations
- Cache hit rate and cache coherence (false sharing, stride patterns)
- SIMD-friendly data layouts

## Benchmarks

Performance tests live in `kairos_engine/benches/` using the `criterion` crate. Each format family (uncompressed, BC, ETC2, ASTC) should have its own benchmark group covering both encode and decode throughput for representative sizes.
