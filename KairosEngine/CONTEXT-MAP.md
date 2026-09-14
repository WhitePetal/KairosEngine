# Context Map

## Contexts

- [Asset system](./kairos_asset/CONTEXT.md) — loading, storing, and handing out assets by handle
- [Graphics](./kairos_graphics/CONTEXT.md) — textures, materials, meshes, shaders, and the rendering pipeline
- [Editor](./kairos_editor/CONTEXT.md) — the host and authoring tool built on the engine

## Relationships

- **Graphics → Asset**: materials, meshes, textures, and shaders are assets; graphics consumes `Handle` and the asset store for them.
- **Editor → Asset**: the editor loads and edits assets through the asset server — its own text/toml stores, the texture-edit store, and the asset inspectors.
- **Editor → Graphics**: the editor renders the game window through the graphics pipeline and previews textures, meshes, and materials in the asset inspectors.
