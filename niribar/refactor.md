  Highest-Value Improvements

  1. Split App into smaller controllers

     src/app.rs is 4,580 lines and currently owns window lifecycle, module dispatch, tray menu logic, keyboard navigation, animation, hotplug handling, and view assembly. This is the biggest
     maintainability risk.

     I would split it roughly into:
      - app/update.rs: top-level message dispatch
      - app/surfaces.rs: surface lookup, spawning, output hotplug
      - app/navigation.rs: focused module and popup navigation
      - app/tray.rs: tray update/menu actions
      - app/view.rs: bar and popup rendering
      - app/state.rs: App, SharedState, helper state structs

     The goal is not abstraction for its own sake; it is to make each behavior testable without scanning a 2000-line match.
  2. Harden the IPC socket

     src/ipc.rs:17 falls back to /tmp/niribar.sock, and src/ipc.rs:33 removes any stale path before binding. That makes local control more permissive than it needs to be.

     Suggested changes:
      - Refuse to start IPC if XDG_RUNTIME_DIR is missing, or create a user-private directory under /tmp with strict ownership and mode.
      - Set socket permissions to 0600 after bind.
      - Before unlinking a stale path, verify it is a socket and owned by the current user.
      - Consider including UID or namespace in the socket name if multiple instances are possible.

     This matters because toggle-expand is low-impact today, but IPC tends to grow.
  3. Constrain user-configured shell execution

     The script module runs sh -c and buffers all stdout via .output() at src/modules/script.rs:59. The button module also runs sh -c and drops the child handle at src/modules/button.rs:77.

     If shell commands are an intentional feature, keep them, but make the risk explicit and bounded:
      - Add a timeout for scripts.
      - Cap captured stdout size.
      - Optionally support structured argv config in addition to shell strings.
      - For buttons, either wait/reap in a background task or intentionally detach so repeated clicks do not create zombie processes.
      - Rate-limit button activation or ignore clicks while a previous command is still spawning.

     This improves security and performance without removing the power-user workflow.
  4. Add reconnection/backoff to service subscriptions

     The module trait contract says D-Bus-backed modules should recover if services restart, but several subscriptions terminate after an unavailable/error state. Examples: UPower in src/dbus/
     upower.rs, NetworkManager/networkd in src/dbus/networkmanager.rs, and Niri IPC in src/niri/subscription.rs.

     I would introduce a shared retry helper:
      - Initial connect attempt
      - Emit Unavailable on failure
      - Retry with capped exponential backoff
      - Emit fresh state when service returns

     That would make suspend/resume, compositor restart, D-Bus service restart, and network daemon restart much less brittle.
  5. Bound untrusted D-Bus tray data

     Tray items and DBusMenu data come from other local processes. In src/dbus/sni_host.rs, fetch_menu requests the full recursive menu layout, and pixmap data is accepted as provided.

     Suggested limits:
      - Cap menu item count and label length.
      - Avoid full recursive menu fetch until nested menus are actually rendered.
      - Validate pixmap dimensions and byte length before argb_to_rgba.
      - Cache named icon path resolution instead of walking icon directories on every render.

     This is both a security hardening and a performance win.
  6. Cache effective layout data

     view() rebuilds effective zone maps and scans modules repeatedly in src/app.rs:1575 and navigation rebuilds similar lists later. This is fine for small configs, but tray icons and
     multiple bars make the repeated scans more noticeable.

     A good middle ground:
      - Maintain a per-bar/per-surface EffectiveLayout after config load and after TrayUpdated.
      - Store ordered visible module IDs by zone.
      - Use it for rendering and keyboard navigation.

     This also removes duplicated logic between view() and effective_nav_list().
  7. Replace panic-based constructor assumptions

     src/modules.rs:178 and src/modules.rs:206 panic if a constructor receives the wrong params. It is “programming error” today, but this boundary sits close to user config parsing.

     Prefer returning anyhow::Result<Box<dyn Module>> from constructors and failing config load with a clean error. It will make future module additions safer.
  8. Tighten CI gates

     Clippy currently reports warnings, mostly in tests, and the production warnings are type-complexity in src/dbus/sni_host.rs. I’d add CI with:
      - cargo fmt --check
      - cargo test
      - cargo clippy --all-targets --all-features -- -D warnings
      - optionally cargo deny for dependency/license/advisory checks

     Before enabling -D warnings, clean up the existing test warnings and add type aliases for DBusMenu/SNI tuple types.

  Suggested Priority Order

  1. Harden IPC socket and shell-command execution.
  2. Add reconnection/backoff for Niri and D-Bus subscriptions.
  3. Split App by responsibility.
  4. Cache tray icon lookup and effective layouts.
  5. Add CI gates and dependency auditing.

  The codebase already has strong unit-test coverage for behavior, especially config, state transitions, navigation, and surface handling. The next step is making the runtime boundaries safer
  and reducing the central App file so future features do not all land in the same place.
