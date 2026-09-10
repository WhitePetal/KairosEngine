# Context Map

## Contexts

- [Asset system](./kairos_asset/CONTEXT.md) — loading, storing, and handing out assets by handle
- [Graphics](./kairos_graphics/CONTEXT.md) — textures, materials, meshes, shaders, and the rendering pipeline

## Relationships

- **Graphics → Asset**: materials, meshes, textures, and shaders are assets; graphics consumes `Handle` and the asset store for them.
