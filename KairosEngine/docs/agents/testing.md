# Testing After Implementation

What to run to verify a change in this workspace, and when. Read this before choosing a test command — the wrong choice costs 22x.

## Rule: test the crate you changed

```sh
cargo test-crate kairos_ecs
```

Defined in `.cargo/config.toml` as `cargo test --lib --bins --tests --no-fail-fast -p`. The crate name is the only argument, and you already know it — it is the crate whose files you edited. A trailing test-name filter also works: `cargo test-crate kairos_ecs filtered_backtrace`.

**Do not run bare `cargo test` from the workspace root.** This is a virtual workspace, so a root-level `cargo test` compiles and tests all 11 member crates, aborts on the first failure, and includes the doctests. That is the 183-second path described below, and it is never what you want after an edit.

If you would rather not name the crate, `cd` into it and run the command there — inside a crate directory cargo scopes to that crate only. From the workspace root the identical command tests all 11.

## Commands

| Command | Tests | Cost | Use for |
| --- | --- | --- | --- |
| `cargo test-crate <crate>` | that crate only | **~8s** | after every edit — the default |
| `cargo test-fast` | all crates, no doctests | **~12s** | one-pass check across the workspace |
| `cargo check --workspace --all-targets` | compiles, runs nothing | **~4s** | catching downstream compile breakage |
| `cargo test-full` | all crates + doctests | **~183s** | not your job — see below |

Figures are warm-cache after touching `kairos_ecs`, on macOS, with `opt-level = 1` and line-table debug info.

## Where the time actually goes

Two independent costs, needing two different fixes.

**1. Doctests: 453 tests, 173s of a 183s run.** That is 82–95% of the full suite depending on cache state. They verify documentation examples. Almost no edit can affect them, so they belong at the merge gate. Every command above except `test-full` skips them via `--lib --bins --tests`.

**2. Recompiling dependents.** `kairos_ecs` sits at the base of the graph:

```
kairos_asset         ← kairos_collections
kairos_collections   leaf
kairos_math          leaf
kairos_ptr           leaf
kairos_supervisor    leaf
kairos_tasks         leaf
kairos_ecs           ← kairos_tasks, kairos_ptr, kairos_collections
├── kairos_time      ← kairos_ecs
├── kairos_transform ← kairos_ecs, kairos_math
├── kairos_graphics  ← kairos_asset, kairos_ecs, kairos_math, kairos_transform (+ wgpu, egui, winit, image)
└── kairos_engine    ← kairos_asset, kairos_graphics and all of the above (+ wgpu, egui, winit, rapier3d)
```

Touching `kairos_ecs` invalidates `kairos_time`, `kairos_transform`, `kairos_graphics` and `kairos_engine`. Scoping with `-p` skips that rebuild: 3.9s of compiling instead of 9.6s, ~8s wall instead of ~12s. `kairos_engine` is the expensive one — it pulls in wgpu, egui, winit and rapier3d.

The ~920 unit tests themselves execute in well under a second. Test execution is never the problem.

## Safety net for shared crates

Scoping has one deliberate blind spot: `cargo test-crate kairos_ecs` says nothing about whether `kairos_engine` still compiles or passes. When you change a crate that others depend on, add the cheap check:

```sh
cargo test-crate kairos_ecs
cargo check --workspace --all-targets
```

That pair costs ~12s and catches compile breakage across every dependent, `kairos_engine` included. It does not run the dependents' tests — if you changed an API that `kairos_engine` consumes, run `cargo test-crate kairos_engine` too, or simply use `cargo test-fast`.

## Do not run `cargo test-full`

`cargo test-full` takes ~183s. **Do not run it to verify your own work.** It is the merge gate, not an implementation step. If you believe it is genuinely required, say so and let the user decide.

A full run cannot tell you anything that `cargo test-crate` plus `cargo check --workspace --all-targets` does not, except whether documentation examples still compile — and those are a property of doc comments, not of the implementation you just wrote.

## Finding which crates changed

```sh
git diff --name-only | cut -d/ -f1 | sort -u
```

This lists the top-level directories you touched; each `kairos_*` entry is a crate name to hand to `cargo test-crate`. Remember the graph above: changing `kairos_ecs` means `kairos_time`, `kairos_transform` and `kairos_engine` are affected consumers even though their files are untouched.

## Notes

- **`RUST_BACKTRACE`** is set to `1` in `.cargo/config.toml`, so `kairos_ecs`'s `filtered_backtrace_test` passes without extra ceremony. That test panics by design when the variable is missing or `0`, and under fail-fast it used to abort the whole workspace run before any downstream crate was tested. Override on the command line if you need `full`.
- **Doctests are documentation.** Keep them green and fix them when they break, but they are the wrong thing to pay three minutes for on every edit.
- **There is no CI in this repo.** The full suite is therefore the only pre-merge gate and it runs wherever someone remembers to run it. If CI is added, `cargo test-full` belongs there and this file should say so.
