# Code Quality Refactor Summary

## Completed

- Restored clean `cargo clippy --all-targets -- -D warnings`.
- Extracted menu popup reducer from `App::update`.
- Extracted tray reducer from `App::update`.
- Added typed `ModuleKind`.
- Wrapped runtime modules in `ModuleStore`.
- Shared dynamic module cleanup.
- Centralized command execution.
- Bounded tray pixmap and DBusMenu data.
- Isolated PulseAudio unsafe callback.
- Reduced KDL parser duplication.

## Verification

- `rtk cargo test`
- `rtk cargo clippy --all-targets -- -D warnings`
- `rtk cargo build --release`

## Code Size Snapshot

Captured with `rtk wc -l src/app.rs src/app/*.rs src/modules.rs src/config/parse.rs src/dbus/sni_host.rs`.

```text
3723 app.rs
159 app/animation.rs
24 app/dynamic_modules.rs
200 app/focus.rs
51 app/layout.rs
83 app/menu.rs
116 app/module_events.rs
75 app/module_state.rs
111 app/module_store.rs
115 app/navigation.rs
182 app/niri.rs
113 app/perf.rs
113 app/subscriptions.rs
181 app/surfaces.rs
75 app/tray.rs
274 app/view.rs
25 app/widget_ops.rs
389 modules.rs
1665 config/parse.rs
945 dbus/sni_host.rs
Σ 8619
```

`src/app.rs` is 56 lines smaller than the original 3779-line baseline. The main improvement is structural: menu and tray behavior now live in focused `src/app/menu.rs` and `src/app/tray.rs` modules instead of the central update router.

## Residual Risk

- Runtime tray and PulseAudio behavior still needs manual Wayland-session smoke testing.
- Command execution remains intentionally shell-based for compatibility, but now has one audit boundary.
