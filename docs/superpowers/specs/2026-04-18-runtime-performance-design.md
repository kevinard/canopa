# Runtime Performance Design

## Goal

Reduce idle CPU usage and activity spikes in niribar without changing user-visible bar behavior.
The first pass targets app-level churn that is visible in the current codebase: subscription
identity collisions, repeated no-op state writes, high-frequency cursor messages, and full module
rebuilds when event data has not changed.

## Scope

This design covers conservative runtime changes only. It does not redesign rendering, replace
Iced, remove modules, or change configuration semantics. The existing `tiny-skia` dependency
work in `Cargo.toml` and `Cargo.lock` is treated as separate in-progress work.

## Approach

Use stable, distinct subscription keys for all long-lived subscriptions. Niri IPC and local IPC
currently both use `Subscription::run_with(())`; they should instead use named key types, matching
the D-Bus subscription pattern already documented in `src/dbus.rs`.

Suppress no-op state updates. Module and system event handlers should compare incoming values with
the current shared state and only mutate when the visible value changes. This applies to clock,
script, battery, network, volume, focused-window, and D-Bus readiness where equality is available
or can be added cheaply.

Reduce high-frequency input churn. Cursor position is only needed to anchor tray menus, so cursor
movement handling should avoid storing duplicate positions and should be disabled when no tray
module is configured.

Avoid unnecessary dynamic module rebuilds. Workspace item and tray icon rebuild paths should skip
work when a new snapshot is equivalent to the current dynamic module set.

## Data Flow

Subscriptions emit `Message` values into `App::update`. The update handlers decide whether the
message changes visible state. If not, they return `Task::none()` without mutating app state.
When visible state changes, the existing view path remains responsible for rebuilding the Iced
element tree.

## Testing

Add focused unit tests before implementation:

- Distinct subscription key behavior where it can be asserted locally.
- No-op module events preserve existing state and do not perform redundant inserts.
- Cursor movement is ignored when no tray is configured and duplicate positions are ignored.
- Workspace rebuild skips equivalent workspace lists.

After unit tests pass, run the project test suite. Manual runtime verification should compare idle
CPU before and after on the same Niri session and configuration.

## Risks

Equality checks must compare visible state, not internal identity. Suppressing an update that affects
layout or focus would create stale UI, so each skip must be narrow and covered by tests.

Subscription key changes should preserve subscription lifetime semantics. Keys must remain stable
across `App::subscription()` calls, but distinct across unrelated subscriptions.
