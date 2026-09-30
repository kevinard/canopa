# Story 8.3: Ship and Embed Documented Default Configuration

Status: review

## Story

As a new user installing niribar,
I want a well-documented default config file available at install time,
so that I have a working, readable starting point without needing to write config from scratch.

## Acceptance Criteria

1. **Given** the niribar repository
   **When** the binary is built
   **Then** `crates/niribar/config/default.kdl` is embedded in the binary as the fallback config (used when no user config exists)

2. **Given** `crates/niribar/config/default.kdl`
   **When** it is present in the repository
   **Then** it is also available as a standalone file for users to copy and customize

3. **Given** the default config file
   **When** inspected
   **Then** it includes all 6 default modules — workspaces, focused-window, volume, network, battery, clock — with zone assignments matching UX-DR18:
   - `workspaces` → top zone
   - `focused-window` → center zone
   - `volume`, `network`, `battery`, `clock` → bottom zone (in that order)

4. **Given** the default config file
   **When** inspected
   **Then** every configurable field is annotated with a comment explaining its purpose and accepted values

5. **Given** the default config file
   **When** parsed with `parse::parse(defaults::DEFAULT_CONFIG)`
   **Then** it succeeds with zero errors, and the parsed `Config` reflects the expected modules and zone assignments

## File Map

| File | Action | What changes |
|------|--------|--------------|
| `crates/niribar/config/default.kdl` | **REWRITE** | Full documented config with all 6 modules, zone layout, commented fields |
| `crates/niribar/src/config.rs` | **MODIFY** | Extend `parse_default_config_succeeds` test to assert module count and zone assignments |

## Tasks / Subtasks

- [x] Task 1: Rewrite `crates/niribar/config/default.kdl` with comprehensive content (AC: #1–4)

  Replace the entire content of `crates/niribar/config/default.kdl` with the following.

  **Important:** The file currently begins with the systemd service comment block added by Story 8.1. That block must be preserved exactly at the top. The KDL content below goes AFTER it — replace everything from `// niribar default configuration` onward.

  New full file content:

  ```kdl
  // To start niribar automatically with your Wayland session:
  //
  //   # Copy the service file (if not installed by your package manager)
  //   cp packaging/systemd/niribar.service ~/.config/systemd/user/
  //
  //   # Enable and start the service
  //   systemctl --user enable --now niribar.service
  //
  // The service starts after graphical-session.target and restarts on failure.
  //
  // niribar default configuration
  // Copy to $XDG_CONFIG_HOME/niribar/config.kdl (or ~/.config/niribar/config.kdl) to customise.

  // ─── Bar ─────────────────────────────────────────────────────────────────────
  // Global bar settings. All fields can be overridden per output — see the
  // "output" section at the bottom for examples.
  bar {
      // Which screen edge the bar anchors to.
      // Accepted values: "left" (default), "right", "top", "bottom".
      edge "left"

      // Bar width in logical pixels when collapsed. Minimum: 2.
      // The compositor multiplies by the output scale factor for physical rendering.
      collapsed-width 48

      // Bar width in logical pixels when expanded. Must be >= collapsed-width.
      // Set equal to collapsed-width to disable the expand/collapse feature entirely.
      expanded-width 200

      // Whether expand/collapse is active.
      // Accepted values: #true (default), #false (permanently collapsed).
      expand #true

      // What user action expands or collapses the bar.
      // Accepted values: "click" (default), "hover".
      expand-trigger "click"

      // Milliseconds of cursor dwell before hover expands the bar.
      // Only relevant when expand-trigger is "hover". Default: 0 (expand immediately).
      hover-delay 0

      // Keyboard shortcut to expand/collapse while the bar has keyboard focus.
      // Optional. Format: modifier+key, e.g. "Super+e" or "Ctrl+b".
      // Also documents the compositor keybind to bind to `niribar --toggle-expand`
      // for global (out-of-focus) toggling.
      // expand-keybind "Super+e"

      // Expand/collapse animation timing.
      animation {
          // Duration of the expand animation in milliseconds. Default: 280.
          expand-ms 280
          // Duration of the collapse animation in milliseconds. Default: 200.
          collapse-ms 200
      }

      // Zone layout — assign module names to top, center, and bottom zones.
      // Names must match the module declarations in the "modules" section below.
      // Multiple modules per zone: list them as separate string arguments.
      modules {
          // Top zone: stacks from the leading edge of the bar (top for vertical bars).
          top "workspaces"
          // Center zone: centered in the remaining space between top and bottom zones.
          center "focused-window"
          // Bottom zone: stacks from the trailing edge of the bar.
          // List multiple modules as separate string arguments on the same line.
          bottom "volume" "network" "battery" "clock"
      }
  }

  // ─── Modules ─────────────────────────────────────────────────────────────────
  // Declare each module referenced in bar.modules above.
  // Module names must match exactly. Type is inferred from the name when the name
  // matches a known type (workspaces, focused-window, battery, clock, network,
  // volume, script, button). For custom names, add a "type" child node.
  modules {
      // workspaces: Niri workspace indicators. Click or scroll to switch workspace.
      // Optional: assign labels and icons to specific workspace numbers (1-based).
      module "workspaces" {
          // workspace 1 {
          //     label "main"   // Short text label shown in expanded state.
          //     icon  "󰣇"     // Icon shown in collapsed state (Nerd Font glyph).
          // }
          // workspace 2 {
          //     label "web"
          //     icon  "󰖟"
          // }
      }

      // focused-window: shows the title of the focused window on this output.
      // No configurable parameters.
      module "focused-window"

      // volume: audio output level indicator. Scroll on the module to adjust volume.
      module "volume" {
          // Volume adjustment per scroll event, as a percentage. Range: 1–100. Default: 5.
          step 5
      }

      // network: network connectivity status (connected/disconnected/interface type).
      // No configurable parameters.
      module "network"

      // battery: charge level and charging/discharging state.
      module "battery" {
          // Percentage at or below which the critical badge and alert color appear.
          // Range: 0–100. Default: 15.
          critical-threshold 15
      }

      // clock: current time and date display.
      module "clock" {
          // Time format string using chrono strftime syntax.
          // Default: "%H:%M" (24-hour, e.g. "14:30"). Use "%I:%M %p" for 12-hour.
          time-format "%H:%M"
          // Date format string using chrono strftime syntax.
          // Default: "%a %d %b" (e.g. "Mon 12 Apr").
          date-format "%a %d %b"
      }
  }

  // ─── Per-output overrides ─────────────────────────────────────────────────────
  // Add "output" blocks to override any bar field for a specific monitor.
  // The output name must match the Wayland output name — run `niri msg outputs`
  // to list connected output names.
  //
  // Any bar field can be overridden: edge, collapsed-width, expanded-width, expand,
  // expand-trigger, hover-delay, expand-keybind, animation, modules.
  //
  // Use `enabled #false` to suppress the bar on a specific output entirely.
  //
  // Example: horizontal bar on a secondary monitor
  // output "DP-2" {
  //     edge "top"
  //     collapsed-width 32
  //     expanded-width 0
  //     modules {
  //         top "workspaces"
  //         center "focused-window"
  //         bottom "clock"
  //     }
  // }
  //
  // Example: disable bar on a specific output
  // output "HDMI-A-1" {
  //     enabled #false
  // }
  ```

- [x] Task 2: Extend `parse_default_config_succeeds` test in `crates/niribar/src/config.rs` (AC: #3, #5)

  Locate the existing test `parse_default_config_succeeds` in `crates/niribar/src/config.rs` (currently in the `#[cfg(test)] mod tests` block around line 438) and extend it with module count and zone assertions:

  Replace:
  ```rust
  #[test]
  fn parse_default_config_succeeds() {
      let config = parse::parse(defaults::DEFAULT_CONFIG).unwrap();
      assert_eq!(config.bar.collapsed_width, 48);
      assert_eq!(config.bar.expanded_width, 200);
  }
  ```

  With:
  ```rust
  #[test]
  fn parse_default_config_succeeds() {
      let config = parse::parse(defaults::DEFAULT_CONFIG).unwrap();
      assert_eq!(config.bar.collapsed_width, 48);
      assert_eq!(config.bar.expanded_width, 200);
      // UX-DR18: zone assignments
      assert_eq!(config.bar.modules.top, vec!["workspaces"]);
      assert_eq!(config.bar.modules.center, vec!["focused-window"]);
      assert_eq!(
          config.bar.modules.bottom,
          vec!["volume", "network", "battery", "clock"]
      );
      // All 6 default modules declared
      assert_eq!(config.modules.len(), 6);
      let names: Vec<&str> = config.modules.iter().map(|m| m.name.as_str()).collect();
      assert!(names.contains(&"workspaces"));
      assert!(names.contains(&"focused-window"));
      assert!(names.contains(&"volume"));
      assert!(names.contains(&"network"));
      assert!(names.contains(&"battery"));
      assert!(names.contains(&"clock"));
  }
  ```

## Dev Notes

### Embedding is already wired — AC #1 is pre-satisfied

`crates/niribar/src/config/defaults.rs` already contains:
```rust
pub const DEFAULT_CONFIG: &str = include_str!("../../config/default.kdl");
```

And `crates/niribar/src/config.rs`'s `load()` function already falls back to `defaults::DEFAULT_CONFIG` when no user config exists (line ~354). No Rust changes required for the embedding mechanism itself.

### Zone layout syntax — one line per zone, multiple positional args

The `parse_bar_modules_from_doc` function iterates zone nodes and collects ALL positional args per node into a `Vec<String>`, then assigns the LAST occurrence of each zone name to `result.top/center/bottom`. Multiple modules in a zone go on the **same line** as additional string arguments:

```kdl
// CORRECT — all bottom modules on one line:
bottom "volume" "network" "battery" "clock"

// WRONG — each bottom line overwrites the previous; only "clock" ends up in bottom zone:
bottom "volume"
bottom "network"
bottom "battery"
bottom "clock"
```

### `workspaces` module with commented-out children

The block `module "workspaces" { // comments only }` is valid KDL: `parse_workspaces_params` receives a children doc with no nodes, loops over nothing, and returns `WorkspacesConfig::default()` (all workspaces show number only). This is the desired behavior for the default config.

### `module "focused-window"` has no configurable params

In the parser, `focused-window` falls through to `ModuleParams::None`. Do NOT invent child nodes for it — the module declaration is just `module "focused-window"` with no braces.

### Test update: no new test count change

Task 2 modifies the existing `parse_default_config_succeeds` test in place — it does NOT add a new test function. Test count remains 377.

### `crates/niribar/config/default.kdl` — preserve Story 8.1 header

Story 8.1 prepended the systemd service comment block to `crates/niribar/config/default.kdl`. That block must remain intact at the very top of the file. Task 1 rewrites the entire file, so include that header verbatim at the top.

### KDL validity check

After writing `crates/niribar/config/default.kdl`, verify it is syntactically valid KDL by running `cargo test parse_default_config_succeeds` — this runs the actual runtime parser on the embedded file content. If parsing fails, the test fails with the parser error message.

### Project Structure Notes

```
config/
└── default.kdl         # embedded via include_str! in crates/niribar/src/config/defaults.rs (FR42)
src/
└── config/
    └── defaults.rs     # pub const DEFAULT_CONFIG: &str = include_str!("../../config/default.kdl")
    └── parse.rs        # parse_bar_modules_from_doc() — zone syntax reference
```

### References

- [Source: epics.md, Epic 8, Story 8.3 — acceptance criteria]
- [Source: epics.md, line 113 — UX-DR18 zone assignments]
- [Source: architecture.md — FR39–43 packaging/distribution; AR11: crates/niribar/config/default.kdl embedded in binary]
- [Source: crates/niribar/src/config/defaults.rs — embedding mechanism]
- [Source: crates/niribar/src/config.rs, line ~350 — load() fallback to DEFAULT_CONFIG]
- [Source: src/config/parse.rs, parse_bar_modules_from_doc() — zone parsing logic]

## Dev Agent Record

### Agent Model Used

claude-sonnet-4-6

### Debug Log References

### Completion Notes List

- Rewrote `crates/niribar/config/default.kdl`: full documented config with all 6 modules (workspaces, focused-window, volume, network, battery, clock); zone layout per UX-DR18 (top/center/bottom); every bar and module field annotated with purpose and accepted values; Story 8.1 systemd service header preserved at top
- Extended `parse_default_config_succeeds` in `crates/niribar/src/config.rs`: added zone assertions (`top/center/bottom`) and module count + name assertions; test still modifies the single existing function (no new test added)
- No embedding mechanism changes needed — `crates/niribar/src/config/defaults.rs` `include_str!` and `config::load()` fallback were already correct
- `cargo test`: 402 passed, 0 failed

### File List

- `crates/niribar/config/default.kdl`
- `crates/niribar/src/config.rs`

## Change Log

| Date | Version | Description | Author |
|------|---------|-------------|--------|
| 2026-04-12 | 1.0 | Implemented: full documented default.kdl with 6 modules + UX-DR18 zones; extended parse_default_config_succeeds test | claude-sonnet-4-6 |
