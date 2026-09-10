# Testing After Implementation

What to run to verify a change in this workspace, and when. The goal is to keep verification cheap enough that you run it after every edit instead of saving it up.

## Default: `cargo test-fast`

```sh
cargo test-fast
```

Defined in `.cargo/config.toml` as `cargo test --workspace --lib --bins --tests --no-fail-fast`.

Two properties make it the default:

- **It skips doctests.** The 453 doctests in `kairos_ecs` account for 82–95% of a full run. They verify documentation examples, not implementation behaviour, so they belong at the merge gate.
- **It never aborts early.** Plain `cargo test` is fail-fast: the first failing test stops cargo and every downstream crate silently never runs. You then get no signal about `kairos_time`, `kairos_transform` or `kairos_engine` at all.

## Measured cost

macOS, `opt-level = 1` with line-table debug info, nothing else running.

Test execution with nothing to recompile:

| Command | Time |
| --- | --- |
| `cargo test-fast` | **0.5s** |
| `cargo test-full` | **56s – 183s** |

The ~920 unit tests execute in well under a second, always. The suite is not slow — **the doctests are**, and they are the only reason a full run feels expensive. Their cost swings widely with cache state: 56s when their compiled artifacts are warm, 173s right after a shared crate changed and rustdoc has to rebuild all 453 of them.

## Scope to the crate you changed

Recompilation, not execution, dominates iteration. Dependency graph:

```
kairos_collections   leaf
kairos_math          leaf
kairos_ptr           leaf
kairos_supervisor    leaf
kairos_tasks         leaf
kairos_ecs           ← kairos_tasks, kairos_ptr, kairos_collections
├── kairos_time      ← kairos_ecs
├── kairos_transform ← kairos_ecs, kairos_math
└── kairos_engine    ← all of the above (+ wgpu, egui, winit, rapier3d)
```

`kairos_ecs` sits at the base, so touching it invalidates `kairos_time`, `kairos_transform` and `kairos_engine`. `kairos_engine` is the expensive one to rebuild.

While iterating, scope to one crate:

```sh
cargo test -p kairos_ecs --lib
```

That skips the downstream rebuild entirely. Recompile cost for a scoped run is a few seconds; a workspace-wide run after touching `kairos_ecs` costs roughly 5–20s of compiling depending on incremental cache state.

## When to run `cargo test-full`

```sh
cargo test-full
```

Run it before calling a change done when any of these hold:

- You changed a shared crate (`kairos_ecs`, `kairos_collections`, `kairos_math`) that other crates depend on.
- You changed public API, so downstream call sites and their tests are in scope.
- You changed a doc comment containing an example — that example **is** a doctest.
- The change is about to be committed or opened as a PR.

Otherwise `cargo test-fast` after each edit is the expected behaviour. Running the full suite after every edit buys nothing that `cargo test-fast` does not already cover.

## Notes

- **`RUST_BACKTRACE`** is set to `1` in `.cargo/config.toml`, so `kairos_ecs`'s `filtered_backtrace_test` passes without extra ceremony. That test panics by design when the variable is missing or `0`, and under fail-fast it used to abort the whole workspace run before any downstream crate was tested. Override it on the command line if you need `full`.
- **Doctests are documentation.** Keep them green and fix them when they break, but they are the wrong thing to pay minutes for on every edit — particularly because most edits cannot affect a doc example at all.
- **There is no CI in this repo.** The full suite is therefore the only pre-merge gate and it runs wherever someone remembers to run it. If CI is added, `cargo test-full` belongs there and this file should say so.
