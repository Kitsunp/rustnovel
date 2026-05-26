# Dependency convergence snapshot - 2026-05-26

Command:

```text
cargo tree -d
```

Status after the audit implementation pass:

- `image` remains duplicated as `0.24.9` and `0.25.9`.
  - `0.24.9` is pulled by `eframe 0.27.2` and is also still present in GUI dev dependencies.
  - `0.25.9` is used by `vnengine_assets`.
  - Do not force convergence until the GUI stack can move with `eframe`/`egui`; otherwise this only hides a transitive duplicate.
- `toml` remains duplicated as `0.8.23` and `0.9.12`.
  - `0.8.23` is used by core.
  - `0.9.12` is used by GUI.
  - This is a direct convergence candidate, but it should be changed in a dedicated pass with manifest parsing fixtures because TOML 0.9 uses newer `serde_spanned`/datetime dependencies.
- `thiserror` remains duplicated as `1.0.69` and `2.0.18`.
  - `1.0.69` is direct in local crates and pulled by `eframe`.
  - `2.0.18` is transitive through `postcard`, `pixels`, and `wgpu`.
  - Keep until GUI/runtime dependency constraints converge.
- Windows and graphics stack duplicates (`windows`, `windows-core`, `windows-sys`, `raw-window-handle`, `glow`, `glutin_wgl_sys`, `cfg_aliases`, `bitflags`) are transitive through `eframe`, `winit`, `wgpu`, `pixels`, `rodio`, `cpal`, and tooling.
  - Do not pin or override these in this repository without upgrading the owning graphics/audio crates together.

Decision:

- No Cargo dependency versions were changed in this pass.
- The safe deliverable is this snapshot plus the post-change `cargo tree -d`, `cargo test`, and `cargo clippy` acceptance run.
- Direct candidates for a future focused PR: `toml` first, then local `thiserror` once GUI/runtime transitive constraints make that useful.
