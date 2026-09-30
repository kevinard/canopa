# Story 5.1: Establish D-Bus Session and System Bus Connections

Status: review

## Story

As a **developer**,
I want shared zbus connections established at startup for the session and system buses,
so that all D-Bus modules borrow handles rather than each creating their own connection.

## Acceptance Criteria

1. **Given** niribar starts with at least one D-Bus-backed module enabled (`battery`, `network`, or `volume` in config)
   **When** the app initialises
   **Then** exactly one zbus connection is created for the session bus (UPower, WirePlumber)
   **And** exactly one zbus connection is created for the system bus (NetworkManager)
   **And** both connections share the single Tokio runtime (no secondary runtime)

2. **Given** a D-Bus bus is unavailable at startup
   **When** the connection attempt occurs
   **Then** the attempt fails silently — no stderr output, no panic, `None` is stored for that bus
   **And** dependent modules are effectively omitted (they will check `shared.dbus.session/system`)

3. **Given** niribar starts with no D-Bus-backed modules in config
   **When** the app initialises
   **Then** no D-Bus connections are created (subscription is not started)

4. **Given** connections are established
   **When** `Message::DbusReady(connections)` is handled in `update()`
   **Then** `self.shared.dbus` is set to the received `DbusConnections`

5. **Given** niribar is built
   **When** `cargo clippy -- -D warnings` is run
   **Then** zero warnings

6. **Given** niribar is built
   **When** `cargo test` is run
   **Then** all 179 pre-existing tests pass and at least 4 new tests pass

## Tasks / Subtasks

- [x] Task 1: Implement `src/dbus/mod.rs` — `DbusConnections` struct and init subscription (AC: #1, #2, #3)
  - [x] 1.1: Replace the placeholder comment with the `DbusConnections` struct:
    ```rust
    /// Shared D-Bus connections, established once at startup (Story 5.1).
    ///
    /// `zbus::Connection` is `Clone` + Arc-backed — cloning gives a reference to the
    /// same underlying connection. Module subscriptions clone these handles; they
    /// never create their own connections (AR5).
    #[derive(Debug, Clone, Default)]
    pub struct DbusConnections {
        /// Session bus handle for UPower and WirePlumber.
        /// `None` if the session bus was unavailable at startup.
        pub session: Option<zbus::Connection>,
        /// System bus handle for NetworkManager.
        /// `None` if the system bus was unavailable at startup.
        pub system: Option<zbus::Connection>,
    }

    impl DbusConnections {
        /// Attempt to connect to both buses.
        /// Never panics — unavailable buses yield `None` (UX-DR16 silent omission).
        pub async fn connect() -> Self {
            let session = zbus::Connection::session().await.ok();
            let system = zbus::Connection::system().await.ok();
            Self { session, system }
        }
    }
    ```

  - [x] 1.2: Add the one-shot init subscription function after the struct and impl:
    ```rust
    pub fn dbus_init_subscription() -> iced::Subscription<crate::app::Message> { ... }
    ```
    Note: builder closure uses `|_: &DbusInitId|` (fn pointer takes `&D`, not `D` by value — iced_futures 0.14 API).

  - [x] 1.3: Add tests at the bottom of `src/dbus/mod.rs`:
    - `dbus_connections_default_session_is_none`
    - `dbus_connections_default_system_is_none`

- [x] Task 2: Add `dbus` field to `SharedState` in `crates/niribar/src/app.rs` (AC: #4)
  - [x] 2.1: Added `use crate::dbus::DbusConnections;` import
  - [x] 2.2: Added `pub dbus: DbusConnections` to `SharedState`; updated `SharedState::new()`

- [x] Task 3: Add `Message::DbusReady` variant to the Message enum in `crates/niribar/src/app.rs` (AC: #4)
  - [x] 3.1: Added `DbusReady(DbusConnections)` variant after `Niri(...)`

- [x] Task 4: Handle `Message::DbusReady` in `App::update()` (AC: #4)
  - [x] 4.1: Added match arm before `_ => Task::none()` catch-all

- [x] Task 5: Add D-Bus init subscription to `App::subscription()`, gated on config (AC: #1, #3)
  - [x] 5.1: Added `needs_dbus` config check and `dbus_sub` to all branches of `subscription()`

- [x] Task 6: Tests in `crates/niribar/src/app.rs` (AC: #5, #6)
  - [x] 6.1: Added `shared_state_dbus_starts_as_default` and `dbus_ready_stores_connections_in_shared_state`

- [x] Task 7: Quality gates (AC: #5, #6)
  - [x] 7.1: `cargo build` — succeeds
  - [x] 7.2: `cargo clippy -- -D warnings` — zero warnings
  - [x] 7.3: `cargo fmt --check` — passes
  - [x] 7.4: `cargo test` — 183 passed (179 pre-existing + 4 new)

## Dev Notes

### What Exists After Story 4.4

- `src/dbus/mod.rs`: contains only `// placeholder — Story 5.1 implements zbus connection management here`
- `crates/niribar/src/app.rs`:
  - `SharedState` has `modules` and `niri` fields only — no `dbus` field yet
  - `Message` enum has no `DbusReady` variant
  - `App::subscription()` runs `module_subs` + `niri_sub` only
  - `update()` has `_ => Task::none()` catch-all at the end
- `Cargo.toml`: `zbus = "5.12.0"` already present — no Cargo.toml changes needed
- `src/modules/mod.rs`: `KNOWN_MODULE_NAMES` already includes `"battery"`, `"network"`, `"volume"`, but `build_registry()` does NOT register them — Stories 5.2–5.4 add those
- `ModuleState` has no Battery/Network/Volume variants — Story 5.1 does NOT add them
- 179 tests passing

### Architecture Requirements (AR5, AR4)

**AR5 (single connection per bus):** Enforced by `dbus_init_subscription()` which:
1. Fires once and terminates (stream returns `None` after the first item)
2. Iced does not restart terminated subscriptions
3. `zbus::Connection` is `Clone` + Arc-backed — cloning gives the SAME underlying connection, not a new one

**AR4 (single Tokio runtime):** `zbus::Connection::session().await` and `zbus::Connection::system().await` both use the executor they're awaited on — which is iced_layershell's Tokio runtime. No secondary runtime is created.

### Subscription Deduplication — Why `DbusInitId` is Needed

`niri_subscription()` calls `Subscription::run_with((), |()| ...)`. If `dbus_init_subscription()` also uses `run_with((), ...)`, Iced deduplicates them as identical (same hash `()`), and only ONE subscription runs. The `DbusInitId` struct gives the D-Bus init subscription a distinct identity:
```rust
#[derive(Hash)]
struct DbusInitId;
```
`DbusInitId` hashes differently from `()`. Architecture rule: "Every subscription uses a distinct ID via a named type."

### `DbusConnections` and Message Enum Trait Bounds

The `Message` enum is declared with:
```rust
#[to_layer_message]
#[derive(Debug, Clone)]
pub enum Message { ... }
```

Adding `DbusReady(DbusConnections)` requires `DbusConnections: Debug + Clone`. Both are satisfied:
- `zbus::Connection: Debug` (zbus 5.x)
- `zbus::Connection: Clone` (Arc-backed, cloning shares the connection)
- `DbusConnections` derives both via `#[derive(Debug, Clone, Default)]`

### `SharedState` Default Requirement

`SharedState` currently derives `#[derive(Default)]`. Adding `dbus: DbusConnections` requires `DbusConnections: Default`. It is — both `Option<zbus::Connection>` fields default to `None`.

### Silent Failure (UX-DR16)

If a bus is unavailable, `zbus::Connection::session().await.ok()` returns `None`. No message is printed to stderr; no panic occurs. Modules that depend on unavailable connections will find `shared.dbus.session == None` or `shared.dbus.system == None` and return `ModuleState::Unavailable`, rendering nothing (UX-DR16).

### What Stories 5.2–5.4 Will Do

Stories 5.2, 5.3, 5.4 will:
1. Create proxy files (`crates/niribar/src/dbus/upower.rs`, `crates/niribar/src/dbus/networkmanager.rs`, `crates/niribar/src/dbus/wireplumber.rs`)
2. Add `BatteryState`, `NetworkState`, `VolumeState` to `ModuleState`
3. Register the modules in `build_registry()`
4. Module subscriptions will clone `shared.dbus.session/system` from `SharedState`
   - **How subscriptions access SharedState:** In `App::subscription()`, `self` is `&App`, which has `self.shared.dbus`. The dbus subscription for each D-Bus module will be called from `App::subscription()`, passing a cloned `Option<zbus::Connection>` via `run_with(self.shared.dbus.session.clone(), |conn| ...)`. Story 5.1 must not change the `Module::subscription()` trait signature — that is Stories 5.2–5.4's concern.

### Config Guard: D-Bus Module Detection

The config guard in `subscription()` checks `self.config.modules` (not `self.modules`):
- `self.config.modules` = raw KDL config entries (all declared modules, including unregistered ones)
- `self.modules` = only modules successfully resolved via `build_registry()` — battery/network/volume are NOT in the registry yet (Stories 5.2–5.4 add them)

Using `self.config.modules` ensures the D-Bus connections are ready even before the module implementations land.

### File Locations

| What | Where |
|------|--------|
| `DbusConnections` struct, `connect()`, `dbus_init_subscription()` | `src/dbus/mod.rs` |
| `use crate::dbus::DbusConnections;` import | `crates/niribar/src/app.rs` — imports section |
| `SharedState.dbus: DbusConnections` field | `crates/niribar/src/app.rs` — `SharedState` struct |
| `Message::DbusReady(DbusConnections)` variant | `crates/niribar/src/app.rs` — `Message` enum |
| `Message::DbusReady` match arm | `crates/niribar/src/app.rs` — `App::update()` |
| Config guard + `dbus_sub` | `crates/niribar/src/app.rs` — `App::subscription()` |
| `dbus_connections_default_*` tests | `src/dbus/mod.rs` — `#[cfg(test)]` block |
| `shared_state_dbus_starts_as_default`, `dbus_ready_stores_connections_in_shared_state` | `crates/niribar/src/app.rs` — `#[cfg(test)]` block |

### References

- Epic 5 §"Story 5.1": user story + 5 ACs
- Architecture §"D-Bus Boundary": `src/dbus/mod.rs` owns session + system bus handles
- Architecture §"Naming Patterns — D-Bus": proxy files `crates/niribar/src/dbus/{upower,networkmanager,wireplumber}.rs` (Stories 5.2–5.4)
- Architecture §"External Integration Points": session bus = UPower + WirePlumber; system bus = NetworkManager
- Architecture AR5: single zbus connection per bus, modules borrow handles
- Architecture AR4: single Tokio runtime
- Architecture §"Subscription ID Pattern": `std::any::TypeId` / named struct for unique IDs
- Architecture §"Communication Patterns": modules emit `{Name}Event::Unavailable` on service absence
- UX-DR16: Unavailable modules render as nothing — no error indicator in bar UI
- `niri/subscription.rs`: reference pattern for `Subscription::run_with` + `stream::unfold` (one-shot variant)
- `src/modules/mod.rs` line 9–16: `KNOWN_MODULE_NAMES` already includes "battery", "network", "volume"

## Dev Agent Record

### Agent Model Used

claude-sonnet-4-6

### Debug Log References

`Subscription::run_with` in iced_futures 0.14 takes `fn(&D) -> S` (function pointer with reference), not `fn(D) -> S`. The builder closure must be `|_: &DbusInitId| { ... }` — adjusted from the story spec's `|_: DbusInitId|`.

### Completion Notes List

- `DbusConnections` struct implemented in `src/dbus/mod.rs` with `session: Option<zbus::Connection>` and `system: Option<zbus::Connection>`, both `#[allow(dead_code)]` until Stories 5.2–5.4 consume them
- `dbus_init_subscription()` uses a local `#[derive(Hash)] struct DbusInitId;` as the unique subscription ID to avoid collision with `niri_subscription()` which uses `()` as its ID
- `stream::unfold(false, ...)` terminates after emitting one `DbusReady` message — enforces AR5 single-connection guarantee
- `SharedState.dbus: DbusConnections` added; `#[derive(Default)]` on `DbusConnections` satisfies `SharedState`'s own `#[derive(Default)]`
- `Message::DbusReady(DbusConnections)` variant added; `DbusConnections: Debug + Clone` (both derived) satisfies `#[to_layer_message] #[derive(Debug, Clone)]` on `Message`
- Config guard uses `self.config.modules` (raw config) not `self.modules` (registry-resolved) because battery/network/volume are not yet in `build_registry()` — this ensures connections are ready when those stories are implemented
- 183 tests total: 179 pre-existing pass + 4 new (2 in `src/dbus/mod.rs`, 2 in `crates/niribar/src/app.rs`)
- `cargo fmt`, `cargo clippy -- -D warnings`, `cargo test` all pass

### File List

- `src/dbus/mod.rs` — replaced placeholder; added `DbusConnections` struct + `connect()` + `dbus_init_subscription()` + 2 tests
- `crates/niribar/src/app.rs` — added `use crate::dbus::DbusConnections`; added `SharedState.dbus` field; added `Message::DbusReady` variant; added `DbusReady` match arm in `update()`; updated `subscription()` with config-gated `dbus_sub`; added 2 tests

### Change Log

- Added `DbusConnections` struct and `dbus_init_subscription()` to `src/dbus/mod.rs` — establishes session and system bus connections once at startup, stored in `SharedState.dbus` (AR5, Story 5.1)
- Extended `SharedState` with `dbus: DbusConnections` field and `Message::DbusReady(DbusConnections)` variant to wire connection results into the Iced update loop
- Subscription gated on `battery`/`network`/`volume` presence in config — no D-Bus connections created when no D-Bus modules are configured
