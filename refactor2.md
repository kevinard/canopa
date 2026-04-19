• I read through the core app, config, modules, D-Bus, Niri, and surface paths. Current baseline: cargo test passes with 474 tests. cargo clippy --all-targets -- -D warnings fails on 4 small
  style issues: one needless borrow in src/app.rs:562 and three collapsible ifs in src/dbus/sni_host.rs:692, src/dbus/sni_host.rs:703, src/dbus/sni_host.rs:717.

  Highest-Value Refactors

  1. Split App::update into domain reducers

  src/app.rs:420 is still doing too much: generic UI messages, tray/SNI commands, menu lifecycle, D-Bus calls, Niri workspace actions, performance reporting, and popup window management. Some
  handlers are already extracted into app/*, but tray/menu logic remains large and stateful in the central match.

  Suggested work:

  - Move tray icon state mutation into app/tray.rs.
  - Move popup/menu lifecycle into app/menu.rs.
  - Keep App::update as a shallow router.
  - Extract repeated “close all menu popups and restore bar keyboard interactivity” code shared by MenuItemSelected and MenuClosed.

  Why it matters: this reduces accidental regressions in the event loop, makes security-sensitive D-Bus actions easier to audit, and creates smaller test targets.

  2. Introduce a ModuleStore / indexed module registry

  The app stores modules as Vec<(Zone, Box<dyn Module>)> at src/app.rs:61, then repeatedly scans it by id/type/config name. Examples include state fan-out in src/app/module_state.rs:16, layout
  expansion in src/app/layout.rs:101, interaction dispatch in src/app/module_events.rs:48, tray insertion, workspace rebuilds, and subscription discovery.

  Suggested work:

  - Wrap the vector in a ModuleStore.
  - Maintain lightweight indexes: id -> index, module_type -> Vec<ModuleId>, config_name -> Vec<ModuleId>.
  - Provide methods like ids_by_type("battery"), module_by_id(id), remove_dynamic_by_prefix("tray:"), insert_after_placeholder(...).
  - Keep ordering explicit because view/layout depend on order.

  Why it matters: fewer O(n) scans per frame/event, less duplication, and a cleaner boundary for dynamic modules like tray icons and workspace items.

  3. Unify dynamic placeholder expansion

  Tray icons and workspace items use the same conceptual pattern: placeholder module, remove generated children, clear stale focus/hover, insert generated modules after placeholder. Tray logic
  lives inside src/app.rs:506, while workspace rebuild logic lives in src/app/niri.rs:123.

  Suggested work:

  - Add a shared DynamicModuleGroup helper.
  - Inputs: placeholder type, child type, child id prefix, builder callback.
  - Centralize stale focus/hover cleanup.
  - Centralize reverse insertion and slot handling.

  Why it matters: this is real duplication in a tricky part of the app. A shared helper would reduce bugs when adding more dynamic module families.

  4. Replace stringly typed module types with a ModuleKind

  Module identity is currently mixed across strings: known types in src/config.rs:6, registry keys in src/modules.rs:134, config parsing in src/config/parse.rs:213, and many runtime checks
  like "tray_icon", "workspace_item", "focused-window".

  Suggested work:

  - Add enum ModuleKind { Battery, Clock, Network, Volume, Workspaces, WorkspaceItem, FocusedWindow, Script, Button, Tray, TrayIcon }.
  - Parse config strings into ModuleKind.
  - Let Module::module_kind() -> ModuleKind.
  - Keep as_str() only for config/user-facing names.

  Why it matters: fewer typo bugs, faster comparisons, better compiler help, and simpler validation/registry code.

  5. Refactor config parsing around typed KDL accessors

  src/config/parse.rs:48 through src/config/parse.rs:541 repeats “get child, get first entry, validate type, produce contextual error” many times. It is already well tested, but the
  duplication makes future options expensive and inconsistent.

  Suggested work:

  - Add small helpers: required_string_child, optional_string_child, optional_u8_range, optional_u32, required_node_arg_string.
  - Preserve current error wording through tests.
  - Replace the if module_type == ... chain with ModuleKind dispatch.

  Why it matters: better maintainability without changing behavior. Also reduces security risk around accepting malformed config silently.

  6. Harden command execution boundaries

  Script and button modules intentionally run shell commands. That is a valid feature, but it is the highest security-sensitive local behavior.

  Relevant spots:

  - src/modules/script.rs:30 runs sh -c.
  - src/modules/button.rs:36 runs sh -c.
  - src/app/module_events.rs:89 runs wpctl.

  Suggested work:

  - Create a command_runner module with shared timeout, output cap, environment policy, and logging.
  - Consider supporting structured commands in config later, e.g. argv list, while keeping shell command compatibility.
  - Add rate limiting/debounce for button commands.
  - Ensure spawned button processes have a clear lifecycle policy.

  Why it matters: centralizing process execution makes security review practical and prevents slightly different subprocess behavior from spreading.

  7. Add explicit size limits to untrusted tray data

  SNI and DBusMenu data comes from other desktop processes. Current code accepts pixmaps, labels, and menu trees with little bounding.

  Relevant spots:

  - Pixmap selection/conversion in src/dbus/sni_host.rs:118.
  - Tray item fetch in src/dbus/sni_host.rs:144.
  - DBusMenu conversion in src/dbus/sni_host.rs:315.
  - Full menu fetch in src/dbus/sni_host.rs:373.

  Suggested work:

  - Reject pixmaps with invalid dimensions or byte lengths not matching width * height * 4.
  - Cap max pixmap dimensions and total bytes.
  - Cap menu item count and label length.
  - Preserve graceful degradation: bad item becomes unavailable/skipped, not fatal.

  Why it matters: this is a direct performance and robustness hardening against buggy or malicious tray clients.

  8. Encapsulate unsafe PulseAudio mainloop handling

  The one meaningful unsafe block is in src/dbus/wireplumber.rs:106. The comment explains the invariant, but the raw pointer is still embedded in a long function.

  Suggested work:

  - Extract a tiny wrapper around the threaded mainloop signal callback.
  - Keep the unsafe block isolated in one function with a narrow contract.
  - Add tests around pure volume math and state translation, leaving FFI integration behind the wrapper.

  Why it matters: small unsafe islands are easier to audit and harder to accidentally invalidate.

  Near-Term Order

  I’d do this sequence:

  1. Fix the 4 clippy failures and make cargo clippy --all-targets -- -D warnings part of the normal quality gate.
  2. Extract app/menu.rs and app/tray.rs from App::update.
  3. Add ModuleKind and migrate runtime string comparisons.
  4. Introduce ModuleStore with indexes.
  5. Unify tray/workspace dynamic module expansion.
  6. Harden command execution and SNI/DBusMenu bounds.

  The code already has good test volume and some performance instrumentation. The best next improvements are not broad rewrites; they are targeted boundary extractions around app event
  routing, module lookup, dynamic module expansion, config parsing, and untrusted external inputs.
