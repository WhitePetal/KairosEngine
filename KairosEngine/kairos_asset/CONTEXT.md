# Asset system

Loading, storing, and handing out assets by handle.

## Language

**Asset**:
A loadable piece of content — texture, mesh, material, sound, and so on — that the engine owns and hands out by handle.
*Avoid*: resource, file

**Asset id**:
An asset's identity, independent of who holds it. A weak handle carries only an id and neither keeps the asset loaded nor proves it exists.
*Avoid*: index, uuid, key

**Handle**:
A reference to an asset. A strong handle keeps the asset loaded while it lives; a weak handle names it without keeping it alive.
*Avoid*: pointer, reference, Arc

**Asset store** (`Assets<A>`):
The collection of loaded values for one asset type, addressed by handle or id.
*Avoid*: asset manager, asset cache

**Asset server**:
The single entry point for loading assets and obtaining handles.
*Avoid*: asset manager

**Asset registration**:
Making an asset type known to the engine before anything loads it, so its store exists from the start rather than appearing on first use.
*Avoid*: lazy setup, on-demand creation

**Asset loader**:
The per-format code that turns bytes plus settings into an asset value.
*Avoid*: importer, parser, decoder

**Asset source**:
A named root that asset paths are resolved against; the default source is the project's asset folder.
*Avoid*: virtual file system, mount

**Asset path**:
An asset's address: a source, a path within that source, and optionally a label.
*Avoid*: file path, URL

**Labeled asset**:
A secondary asset a loader produces alongside its primary asset, addressed by a label appended to the same path.
*Avoid*: sub-asset, child asset, sub-resource

**Asset dependency**:
Another asset that must finish loading before this one is ready. Direct dependencies are declared by the loader; recursive dependencies are all those reached transitively.
*Avoid*: reference, link

**Load state**:
How far an asset has got — not loaded, loading, loaded, or failed. Tracked for the asset itself, its direct dependencies, and its recursive dependencies.
*Avoid*: status, progress

**Meta sidecar**:
A file beside an asset recording which loader handles it and the settings to load it with.
*Avoid*: config, manifest

**Asset event**:
A notification that an asset was added, modified, removed, released, or finished loading together with all its dependencies.
*Avoid*: signal, message
