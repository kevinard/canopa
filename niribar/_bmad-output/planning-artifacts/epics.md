---
stepsCompleted: [1, 2, 3, 4]
status: 'complete'
completedAt: '2026-03-29'
inputDocuments: ['_bmad-output/planning-artifacts/prd.md', '_bmad-output/planning-artifacts/architecture.md', '_bmad-output/planning-artifacts/ux-design-specification.md']
workflowType: 'epics-and-stories'
project_name: 'niribar'
user_name: 'Kevin'
date: '2026-03-29'
---

# niribar - Epic Breakdown

## Overview

This document provides the complete epic and story breakdown for niribar, decomposing the requirements from the PRD, UX Design Specification, and Architecture into implementable stories.

## Requirements Inventory

### Functional Requirements

FR1: The bar renders as a persistent layer-shell surface on a Wayland compositor
FR2: The bar can be positioned on any screen edge, with vertical side placement as the primary use case
FR3: The bar can be displayed on multiple outputs simultaneously
FR4: The bar adapts its size and positioning to each output's resolution and DPI scale factor
FR5: The bar can be excluded from specific outputs via configuration
FR6: The bar collapses to a minimal width displaying only compact/icon content
FR7: The bar expands to a wider state revealing labels and additional content
FR8: The transition between collapsed and expanded states is animated
FR9: Expand/collapse can be triggered by pointer hover
FR10: Expand/collapse trigger, animation, and width targets are configurable
FR11: The bar renders a configurable ordered set of modules
FR12: Modules can be placed in distinct layout zones (top, center, bottom of bar)
FR13: Each module can define distinct visual representations for collapsed and expanded states
FR14: Module data sources update asynchronously without blocking bar rendering
FR15: Modules can respond to user pointer interactions (click, scroll)
FR16: A custom script module executes a shell command and displays its output in the bar
FR17: A workspace module displays the current state of all Niri workspaces in real time
FR18: The workspace module reflects Niri's scrollable-tiling workspace ordering and structure
FR19: The workspace module supports user-defined labels and icons per workspace
FR20: A focused window module displays the title of the currently focused window in real time; it is an optional module, not a core bar component
FR21: Niri-specific modules are isolated and degrade gracefully on non-Niri compositors
FR22: A battery module displays current battery charge level and charging state
FR23: A network module displays current network connectivity status
FR24: A volume module displays current audio output volume level
FR25: A clock module displays the current date and time
FR26: The clock module supports configurable format strings for date and time display
FR27: The volume module responds to user interaction to adjust output volume
FR28: A button module executes a user-defined action when activated
FR29: The bar can be focused and navigated using keyboard input
FR30: Module interactions (activation, scroll actions) can be triggered via keyboard when the bar is focused
FR31: The expand/collapse state can be toggled via keyboard
FR32: The bar loads configuration from a KDL file at startup
FR33: A complete default configuration is applied when no user config file is present
FR34: The configuration defines which modules are active, their order, and their layout zone placement
FR35: The configuration supports per-output overrides for module sets and bar behavior
FR36: The configuration exposes expand/collapse parameters including trigger mode, animation, and width
FR37: The bar reports actionable error messages when the configuration file is invalid
FR38: The bar reports its version number via a CLI flag
FR39: The project builds via standard `cargo build` with no non-Rust build steps or exotic tooling
FR40: The project provides a systemd user service unit file as an installation artifact
FR41: The project provides a Nix flake as a first-class packaging path
FR42: The project provides a documented default configuration file as an installation artifact
FR43: The bar is functional on wlr-layer-shell-compatible compositors beyond Niri, with Niri-specific modules inactive

### NonFunctional Requirements

NFR1: Expand/collapse animation renders at a consistent frame rate with no perceptible jank — GNOME Shell animations as the quality reference
NFR2: Idle CPU usage is equal to or lower than Waybar on an equivalent module configuration, as measured by published benchmarks
NFR3: Idle memory footprint is equal to or lower than Waybar on an equivalent module configuration, as measured by published benchmarks
NFR4: Niri IPC state changes (workspace transitions, focus changes) are reflected in the bar within one rendered frame — no visible delay between compositor event and bar update
NFR5: Cold start time is not perceptible during a normal Niri session startup
NFR6: D-Bus polling and event handling do not introduce render loop latency spikes — all external data sources run asynchronously
NFR7: Custom script modules and button actions execute with the permissions of the user running niribar — no privilege escalation, no sandboxing imposed
NFR8: niribar transmits no data over the network; all integrations are local IPC (Niri socket, D-Bus)
NFR9: The bar is fully operable without a pointing device — all module interactions reachable via keyboard
NFR10: Default theme colors meet WCAG AA contrast ratio for text and icons against the bar background
NFR11: The bar respects the compositor's HiDPI/scale factor settings — no blurry or incorrectly sized rendering on scaled outputs

### Additional Requirements

- AR1: Project initialized with `cargo new niribar --bin`; Cargo.toml populated with pinned dependency set before any other work
- AR2: Core dependencies: `iced_layershell` 0.13.7, `iced` 0.13.x (pinned to iced_layershell compatibility), `zbus` 5.12.0, `kdl` 6.5.0, `tokio` 1.x (full features)
- AR3: Verify iced_layershell 0.13.7 compiles cleanly with the target Iced 0.13.x patch before any module work begins
- AR4: Single Tokio runtime shared by Iced, zbus, and niri-ipc — no multiple runtimes
- AR5: Single zbus connection per D-Bus bus (session bus for UPower/WirePlumber; system bus for NetworkManager) — modules borrow handles, never create their own connections
- AR6: `SharedState`/`SurfaceState` boundary must be strictly enforced — module data in SharedState only, expand/collapse/focus in SurfaceState only
- AR7: Config fully parsed and validated before any surfaces are created — no lazy config evaluation
- AR8: CI/CD: GitHub Actions — `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test` on PRs; tag-triggered binary release for `x86_64-unknown-linux-gnu`
- AR9: `flake.nix` and `flake.lock` at repo root; CI validates `nix build` on pushes to main
- AR10: `contrib/systemd/niribar.service` provided as installation artifact
- AR11: `config/default.kdl` bundled in repo and embedded in binary as default config

### UX Design Requirements

UX-DR1: Implement `BarSurface` Iced widget with animated width: 280ms cubic-bezier(0.22,1,0.36,1) expand, 200ms cubic-bezier(0.4,0,1,1) collapse; flush default; configurable `bar.gap` and `bar.border-radius` for floating variant
UX-DR2: Implement `ZoneLayout` Iced widget: top/center/bottom zones; center fills remaining space; clips overflow; zones never overlap
UX-DR3: Implement `ModuleFrame` Iced widget: rounded-rect background fill (`color.surface`, 4px radius) on hover and keyboard focus; identical visual treatment for both input methods
UX-DR4: Implement `WorkspacePill` Iced widget: collapsed (32×20px, number only) and expanded (full-width, number + label) states; active state with `color.accent` background
UX-DR5: Implement `ModuleIcon` Iced widget: stateful color (normal per-module color / warning `color.warning` / critical `color.critical` / dim `color.overlay0`); 20px default; SVG source by default
UX-DR6: Implement `ModuleLabel` Iced widget: icon + label left-aligned, value right-aligned; label/value opacity 0→1 with 60ms delay after bar expand animation starts
UX-DR7: Implement `CriticalBadge` Iced widget: overlay badge visible only in collapsed state at configurable threshold; `color.critical` background, `color.base` text; hidden when bar is expanded
UX-DR8: Implement Catppuccin Mocha as default color token set; expose full token override via KDL `[colors]` block and preset shorthand `theme = "catppuccin-mocha|latte|frappe|macchiato"`
UX-DR9: Implement typography: system font default via fontconfig; configurable `font.family`; `font.size.base` 13px, `font.size.small` 11px, `font.size.icon` 16px
UX-DR10: Implement spacing system: 4px base unit; configurable `spacing.module.padding` (8px default), `spacing.module.gap` (4px), `spacing.bar.padding` (4px); `size.bar.collapsed` (48px), `size.bar.expanded` (200px)
UX-DR11: Bundle SVG icon set covering all built-in modules; configurable `icon.font` for icon font fallback; per-module `icon = "name"` override in KDL
UX-DR12: Implement bar edge treatment: single-pixel border (`color.border`) on desktop-facing edge; configurable `bar.border` (bool) and `bar.border-color`
UX-DR13: Expand/collapse uses toggle model: same trigger inverts current state; no modal expansion — keybind always works in both directions
UX-DR14: Module interactions (click, scroll) functional in both collapsed and expanded states — users never need to expand to interact
UX-DR15: Keyboard focus: dedicated keybind entry; arrow key traversal through modules; Tab moves between zones; Escape exits bar and returns focus to previous window
UX-DR16: No loading spinners or skeleton states; modules render empty space at final size until first data arrives; unavailable modules render as nothing (no gap, no placeholder)
UX-DR17: Module critical state badge: configurable threshold per module (default battery ≤15%); badge shows value overlaid on icon in collapsed state only
UX-DR18: Default config ships with 5–6 modules: workspaces (top), focused-window (center, optional), volume + network + battery + clock (bottom) — curated, not comprehensive
UX-DR19: Config error message format: `error: <field.path>: <what was wrong>. Expected: <what was expected>.` — printed to stderr, exit code 1; no partial bar rendered
UX-DR20: Verify and document WCAG AA compliance for all Catppuccin Mocha color token combinations used in niribar; record in ACCESSIBILITY.md

### FR Coverage Map

| FR | Requirement Summary | Epic |
|---|---|---|
| FR1 | Bar renders as persistent layer-shell surface | Epic 1 |
| FR2 | Bar positioned on any screen edge | Epic 1 |
| FR3 | Bar displayed on multiple outputs | Epic 7 |
| FR4 | Bar adapts to output resolution/DPI | Epic 7 |
| FR5 | Bar excludable from specific outputs | Epic 7 |
| FR6 | Bar collapses to minimal width | Epic 3 |
| FR7 | Bar expands to wider state | Epic 3 |
| FR8 | Expand/collapse transition is animated | Epic 3 |
| FR9 | Expand/collapse triggered by hover | Epic 3 |
| FR10 | Expand/collapse parameters configurable | Epic 3 |
| FR11 | Bar renders configurable ordered modules | Epic 2 |
| FR12 | Modules placeable in layout zones | Epic 2 |
| FR13 | Modules define collapsed/expanded states | Epic 2 |
| FR14 | Module data updates asynchronously | Epic 2 |
| FR15 | Modules respond to pointer interactions | Epic 2 |
| FR16 | Custom script module | Epic 2 |
| FR17 | Workspace module (Niri, real time) | Epic 4 |
| FR18 | Workspace module reflects scrollable-tiling order | Epic 4 |
| FR19 | Workspace module supports user labels/icons | Epic 4 |
| FR20 | Focused window module (optional) | Epic 4 |
| FR21 | Niri modules degrade gracefully elsewhere | Epic 4 |
| FR22 | Battery module | Epic 5 |
| FR23 | Network module | Epic 5 |
| FR24 | Volume module | Epic 5 |
| FR25 | Clock module | Epic 2 |
| FR26 | Clock module configurable format strings | Epic 2 |
| FR27 | Volume module responds to interaction | Epic 5 |
| FR28 | Button module | Epic 5 |
| FR29 | Bar keyboard focus and navigation | Epic 6 |
| FR30 | Module interactions via keyboard | Epic 6 |
| FR31 | Expand/collapse toggled via keyboard | Epic 6 |
| FR32 | Config loaded from KDL file | Epic 1 |
| FR33 | Default config applied when none present | Epic 1 |
| FR34 | Config defines modules, order, zones | Epic 1 / Epic 2 |
| FR35 | Config supports per-output overrides | Epic 7 |
| FR36 | Config exposes expand/collapse parameters | Epic 1 / Epic 3 |
| FR37 | Actionable error messages for invalid config | Epic 1 |
| FR38 | Version reported via CLI flag | Epic 1 |
| FR39 | Builds via standard cargo build | Epic 1 |
| FR40 | systemd user service unit file provided | Epic 8 |
| FR41 | Nix flake provided | Epic 8 |
| FR42 | Default config file provided as artifact | Epic 8 |
| FR43 | Functional on other wlr-layer-shell compositors | Epic 4 |

## Epic List

| # | Title | Tagline | FRs Covered |
|---|---|---|---|
| 1 | Project Foundation | A Bar That Renders | FR1, FR2, FR32, FR33, FR34, FR36, FR37, FR38, FR39 |
| 2 | Module System | A Bar That Shows Live Data | FR11, FR12, FR13, FR14, FR15, FR16, FR25, FR26, FR34 |
| 3 | Expand/Collapse | A Bar That Breathes | FR6, FR7, FR8, FR9, FR10, FR36 |
| 4 | Niri Integration | A Bar That Understands Niri | FR17, FR18, FR19, FR20, FR21, FR43 |
| 5 | System Service Modules | A Complete Status Bar | FR22, FR23, FR24, FR27, FR28 |
| 6 | Keyboard Navigation | A Bar Without Borders | FR29, FR30, FR31 |
| 7 | Multi-Output | A Bar For Every Screen | FR3, FR4, FR5, FR35 |
| 8 | Packaging & Distribution | A Bar For The Ecosystem | FR40, FR41, FR42 |

---

## Epic 1: Project Foundation — A Bar That Renders

**Goal:** Deliver a compilable, runnable niribar binary that renders a static bar surface on a Wayland compositor, loads KDL configuration, and provides the foundational scaffolding every subsequent epic builds on.

**FRs covered:** FR1, FR2, FR32, FR33, FR34, FR36, FR37, FR38, FR39

**NFRs relevant:** NFR5 (imperceptible cold start), NFR11 (HiDPI/scale factor)

**ARs relevant:** AR1, AR2, AR3, AR6, AR7, AR11

**UX-DRs relevant:** UX-DR8, UX-DR9, UX-DR10, UX-DR12, UX-DR18, UX-DR19

---

### Story 1.1: Initialize Cargo Project with Pinned Dependencies

As a **developer**,
I want the project initialized with a `Cargo.toml` containing pinned dependencies,
So that the build is reproducible and the iced_layershell/Iced compatibility is verified from day one.

**Acceptance Criteria:**

**Given** an empty repository with a `cargo new niribar --bin` baseline
**When** `cargo build` is run
**Then** the binary compiles successfully with `iced_layershell` 0.13.7, `iced` 0.13.x (iced_layershell-compatible), `zbus` 5.12.0, `kdl` 6.5.0, `tokio` 1.x (full features)
**And** `cargo clippy -- -D warnings` produces no warnings
**And** `cargo fmt --check` passes
**And** a minimal `main.rs` that links against iced_layershell compiles and exits cleanly

---

### Story 1.2: Render a Static Bar Surface on Wayland

As a **user running niribar on a Wayland compositor**,
I want a bar surface to appear at startup positioned on the left screen edge,
So that I have a visible bar anchored to my screen that I can build on.

**Acceptance Criteria:**

**Given** niribar is launched on a wlr-layer-shell-compatible compositor
**When** the binary starts
**Then** a persistent layer-shell surface is created using the wlr-layer-shell protocol
**And** the surface is anchored to the left edge and fills the full screen height by default
**And** the surface renders with the Catppuccin Mocha base color (`#1e1e2e`) as background
**And** a single-pixel border on the right (desktop-facing) edge is rendered in `color.border` (`#45475a`)
**And** the surface remains visible and does not crash under idle conditions
**And** the bar width is 48px (the collapsed default, `size.bar.collapsed`)

---

### Story 1.3: Load KDL Configuration File

As a **user**,
I want niribar to load its configuration from a KDL file,
So that I can customize the bar's behavior without recompiling.

**Acceptance Criteria:**

**Given** a valid KDL config file exists at `$XDG_CONFIG_HOME/niribar/config.kdl`
**When** niribar starts
**Then** the config file is parsed fully before any surface is created
**And** the parsed bar edge position (`bar.edge`) is applied to the surface placement
**And** the parsed bar widths (`size.bar.collapsed`, `size.bar.expanded`) are stored in application state
**And** if the config file does not exist, the embedded default config (`config/default.kdl`) is used transparently
**And** the user sees no error when running on the default config

---

### Story 1.4: Report Actionable Config Errors

As a **user with an invalid config file**,
I want niribar to tell me exactly what is wrong,
So that I can fix my config without guessing.

**Acceptance Criteria:**

**Given** a config file exists with an invalid value (e.g., unknown field, wrong type, missing required value)
**When** niribar starts
**Then** the process exits with code 1 before rendering any surface
**And** a message is printed to stderr in the format: `error: <field.path>: <what was wrong>. Expected: <what was expected>.`
**And** the error identifies the specific field path (e.g., `bar.edge`, `colors.accent`)
**And** at least one invalid-field and one wrong-type scenario are covered by unit tests

---

### Story 1.5: Report Version via CLI Flag

As a **user or package maintainer**,
I want to run `niribar --version` and see the version number,
So that I can verify which release is installed.

**Acceptance Criteria:**

**Given** niribar is installed
**When** `niribar --version` is run
**Then** the version string from `Cargo.toml` is printed to stdout
**And** the process exits with code 0
**And** no Wayland surface is created

---

## Epic 2: Module System — A Bar That Shows Live Data

**Goal:** Deliver the module trait, zone layout, and four concrete modules (clock, script, button, and a stub for future modules) so the bar displays real, updating content and establishes the pattern every later module follows.

**FRs covered:** FR11, FR12, FR13, FR14, FR15, FR16, FR25, FR26, FR34

**NFRs relevant:** NFR2, NFR3 (idle CPU/memory ≤ Waybar), NFR6 (async data — no render loop latency)

**ARs relevant:** AR4 (single Tokio runtime), AR5 (single zbus connection), AR6 (SharedState/SurfaceState boundary)

**UX-DRs relevant:** UX-DR2 (ZoneLayout), UX-DR3 (ModuleFrame hover fill), UX-DR5 (ModuleIcon), UX-DR6 (ModuleLabel), UX-DR14 (interactions in both states), UX-DR16 (no spinners)

---

### Story 2.1: Define the Module Trait and Zone Layout

As a **developer**,
I want a `Module` trait and `ZoneLayout` widget that can host modules in top/center/bottom zones,
So that all subsequent modules have a consistent interface and the bar's spatial structure exists.

**Acceptance Criteria:**

**Given** the module trait is defined in `src/modules/mod.rs`
**When** a concrete type implements `Module`
**Then** it must provide: a `view_collapsed()` and `view_expanded()` Iced element, a `subscription()` returning an Iced subscription for async data, and an `id()` string
**And** `ZoneLayout` renders top/center/bottom zones with center filling remaining space
**And** zones never overlap even when content overflows — overflow is clipped
**And** the layout respects `spacing.bar.padding` (4px default) on all edges
**And** modules within a zone are separated by `spacing.module.gap` (4px default)

---

### Story 2.2: Render Modules from Configuration

As a **user**,
I want the modules listed in my config to appear in the bar in the order and zones I specified,
So that I control what the bar shows and where.

**Acceptance Criteria:**

**Given** the config file specifies a `modules` list with zone placements (e.g., `clock` in bottom zone)
**When** niribar starts
**Then** exactly the configured modules are instantiated, in config order, in their specified zones
**And** modules not listed in config are not rendered
**And** a module with an unrecognized name in config produces an error matching the FR37 format and exits
**And** `ModuleFrame` renders a rounded-rect background fill (`color.surface`, 4px radius) on pointer hover

---

### Story 2.3: Clock Module with Configurable Format

As a **user**,
I want a clock module that shows the current time and date,
So that I always know the time without leaving my workspace.

**Acceptance Criteria:**

**Given** `clock` is in the module list
**When** the bar renders
**Then** the collapsed state shows a time icon and the current time in `font.size.small` (11px)
**And** the expanded state shows an icon + time on one line and the date below it in `ModuleLabel` format
**And** the time and date update each second without blocking the render loop
**And** the format strings are configurable via `clock.time_format` and `clock.date_format` (chrono format syntax)
**And** if no format is configured, defaults of `%H:%M` and `%a %d %b` are used

---

### Story 2.4: Script Module — Shell Command Output in the Bar

As a **user**,
I want to run a shell command and display its stdout in the bar,
So that I can show arbitrary system state the built-in modules don't cover.

**Acceptance Criteria:**

**Given** a `script` module entry with `command = "..."` and `interval = N` (seconds) in config
**When** the bar renders
**Then** the command is executed in a Tokio task at the configured interval
**And** stdout (trimmed) is displayed as the module label in expanded state and truncated in collapsed state
**And** the command runs with the permissions of the niribar process — no privilege escalation
**And** if the command exits non-zero, the previous output is retained and no crash occurs
**And** the render loop is never blocked waiting for the command to complete

---

### Story 2.5: Button Module — User-Defined Clickable Action

As a **user**,
I want a button module that runs a command when I click it,
So that I can launch apps or trigger actions directly from the bar.

**Acceptance Criteria:**

**Given** a `button` module entry with `label = "..."`, `icon = "..."`, and `on_click = "..."` in config
**When** the user clicks the button in either collapsed or expanded state
**Then** the `on_click` command is executed in a Tokio task
**And** the bar does not freeze or stall while the command runs
**And** the collapsed state shows the icon, the expanded state shows icon + label
**And** the button provides visible hover feedback via `ModuleFrame` fill

---

## Epic 3: Expand/Collapse — A Bar That Breathes

**Goal:** The bar animates between a narrow collapsed state and a wider expanded state. Trigger is explicit (pointer click / keybind) by default and optionally hover-based. Animation is smooth and configurable.

**FRs covered:** FR6, FR7, FR8, FR9, FR10, FR36

**NFRs relevant:** NFR1 (animation at consistent frame rate, GNOME Shell as quality reference), NFR6 (no render loop latency during animation)

**ARs relevant:** AR6 (SurfaceState owns expand/collapse state)

**UX-DRs relevant:** UX-DR1 (BarSurface: 280ms expand / 200ms collapse curves), UX-DR6 (label opacity delay), UX-DR7 (CriticalBadge collapsed-only), UX-DR10 (48px / 200px defaults), UX-DR13 (toggle model)

---

### Story 3.1: Bar Collapses and Expands on Explicit Trigger

As a **user**,
I want to click the bar to toggle it between its narrow and wide states,
So that I can see full module labels when I need them and reclaim screen space when I don't.

**Acceptance Criteria:**

**Given** the bar is in collapsed state (48px wide)
**When** the user clicks anywhere on the bar surface
**Then** the bar expands to its configured expanded width (200px default)
**And** clicking again collapses it back to the collapsed width
**And** the toggle model applies — the same action always inverts the current state
**And** expand/collapse state is stored in `SurfaceState` (not `SharedState`)

---

### Story 3.2: Animate the Width Transition

As a **user**,
I want the expand and collapse to animate smoothly,
So that the transition feels polished rather than jarring.

**Acceptance Criteria:**

**Given** the bar is transitioning between collapsed and expanded states
**When** the animation runs
**Then** expand uses a 280ms cubic-bezier(0.22, 1, 0.36, 1) easing curve
**And** collapse uses a 200ms cubic-bezier(0.4, 0, 1, 1) easing curve
**And** module labels animate from opacity 0 to 1 with a 60ms delay after expand begins
**And** module labels disappear immediately on collapse start (no reverse fade delay)
**And** the animation renders without dropped frames under normal system load (NFR1)
**And** the `CriticalBadge` is visible only in collapsed state and hidden immediately when expansion begins

---

### Story 3.3: Hover-Triggered Expand/Collapse

As a **user who wants the bar to open automatically**,
I want the bar to expand when my pointer enters it and collapse when it leaves,
So that I don't need to click to see full module labels.

**Acceptance Criteria:**

**Given** `bar.expand_trigger = "hover"` is set in config
**When** the pointer enters the bar surface
**Then** the bar begins expanding after the configured `bar.hover_delay` (default 0ms)
**And** when the pointer leaves the bar surface, it collapses
**And** if `bar.expand_trigger = "click"` (the default), hover has no effect on expand state
**And** the hover delay is configurable in milliseconds via `bar.hover_delay`

---

### Story 3.4: Configure Expand/Collapse Parameters

As a **user**,
I want to customize the bar's collapse width, expanded width, and animation timing,
So that the bar fits my screen setup and personal preference.

**Acceptance Criteria:**

**Given** the config file specifies `size.bar.collapsed`, `size.bar.expanded`, and/or `bar.animation` values
**When** niribar starts
**Then** the bar uses the configured collapsed width instead of the 48px default
**And** the bar uses the configured expanded width instead of the 200px default
**And** custom animation duration overrides the default 280ms/200ms timings
**And** all expand/collapse config fields have correct defaults when omitted
**And** invalid values (e.g., negative width) produce an error matching the FR37 format

---

## Epic 4: Niri Integration — A Bar That Understands Niri

**Goal:** The bar connects to the Niri IPC socket, subscribes to workspace and focus events, and renders live Niri state — workspaces with labels/icons and the focused window title. Niri modules fail gracefully when running on any other compositor.

**FRs covered:** FR17, FR18, FR19, FR20, FR21, FR43

**NFRs relevant:** NFR4 (IPC state changes reflected within one rendered frame), NFR8 (no network — local IPC only)

**ARs relevant:** AR4 (single Tokio runtime), AR6 (SharedState/SurfaceState boundary)

**UX-DRs relevant:** UX-DR4 (WorkspacePill: 32×20px collapsed / full-width expanded, active `color.accent`), UX-DR16 (unavailable modules render as nothing)

---

### Story 4.1: Connect to Niri IPC and Subscribe to Events

As a **developer**,
I want niribar to establish a connection to the Niri IPC socket and subscribe to its event stream,
So that all Niri-native modules share a single, reliable data source.

**Acceptance Criteria:**

**Given** niribar is running on Niri
**When** the app starts
**Then** a single connection to the Niri IPC socket is established via `niri-ipc`
**And** the connection uses the Tokio runtime (no secondary runtime created)
**And** workspace and focus events from the event stream are dispatched to `SharedState`
**And** if the Niri socket is not found (running on a different compositor), the connection attempt fails silently with no crash or error logged to the user

---

### Story 4.2: Workspace Module — Live Workspace Indicators

As a **user on Niri**,
I want the bar to show all my workspaces as visual indicators,
So that I can see which workspaces exist and which one is active at a glance.

**Acceptance Criteria:**

**Given** `workspaces` is in the module list and niribar is running on Niri
**When** the bar renders
**Then** each workspace is shown as a `WorkspacePill` widget
**And** the active workspace pill has `color.accent` (`#89b4fa`) background
**And** inactive workspace pills have the default surface background
**And** the collapsed state shows pills at 32×20px with the workspace number only
**And** the expanded state shows full-width pills with number and label (if configured)
**And** workspace order matches Niri's scrollable-tiling order (top-to-bottom = top zone to bottom of list)
**And** when a workspace becomes active, the pill updates within one rendered frame (NFR4)

---

### Story 4.3: Workspace Module — User-Defined Labels and Icons

As a **user**,
I want to assign custom labels and icons to my workspaces in config,
So that my workspace indicators are meaningful rather than just numbers.

**Acceptance Criteria:**

**Given** the config contains workspace label/icon assignments (e.g., `workspace 1 { label = "web"; icon = "browser" }`)
**When** the workspace module renders in expanded state
**Then** the configured label is shown alongside the workspace number in the pill
**And** the configured icon is shown in the pill (collapsed and expanded)
**And** workspaces without a configured label show only the number
**And** workspace label/icon config is per-workspace-number, not per-position

---

### Story 4.4: Focused Window Module — Current Window Title

As a **user**,
I want the bar to show the title of the window I'm currently focused on,
So that I can identify the active application without looking at the window itself.

**Acceptance Criteria:**

**Given** `focused-window` is in the module list (center zone by default in the default config)
**When** the user focuses a window
**Then** the window title updates in the module within one rendered frame (NFR4)
**And** the collapsed state shows a window icon only
**And** the expanded state shows the window title truncated to fit the module width
**And** when no window is focused, the module renders nothing (no gap, no placeholder per UX-DR16)

---

### Story 4.5: Graceful Degradation on Non-Niri Compositors

As a **user running niribar on a non-Niri wlr-layer-shell compositor**,
I want Niri-specific modules to disappear quietly,
So that the bar is still usable with all other modules functioning normally.

**Acceptance Criteria:**

**Given** niribar is running on a compositor other than Niri (no Niri IPC socket available)
**When** the bar starts with `workspaces` or `focused-window` in the module list
**Then** those modules are silently omitted from the rendered bar — no gap, no placeholder
**And** all non-Niri modules (clock, battery, etc.) render and function normally
**And** no error is printed to stderr for the missing Niri modules
**And** the bar on Niri continues to function normally (regression guard)

---

## Epic 5: System Service Modules — A Complete Status Bar

**Goal:** Add the three D-Bus-backed system modules — battery, network, and volume (with scroll interaction) — so the bar shows a complete picture of system state in the default configuration.

**FRs covered:** FR22, FR23, FR24, FR27, FR28

**NFRs relevant:** NFR2, NFR3 (idle CPU/memory ≤ Waybar), NFR6 (D-Bus async — no render loop latency), NFR8 (local IPC only)

**ARs relevant:** AR5 (single zbus connection per bus — session bus for UPower/WirePlumber, system bus for NetworkManager), AR4 (single Tokio runtime)

**UX-DRs relevant:** UX-DR5 (ModuleIcon stateful color: normal / warning / critical / dim), UX-DR7 (CriticalBadge at configurable threshold, default battery ≤15%), UX-DR17 (critical state badge per module)

---

### Story 5.1: Establish D-Bus Session and System Bus Connections

As a **developer**,
I want shared zbus connections established at startup for the session and system buses,
So that all D-Bus modules borrow handles rather than each creating their own connection.

**Acceptance Criteria:**

**Given** niribar starts with at least one D-Bus-backed module enabled
**When** the app initialises
**Then** exactly one zbus connection is created for the session bus (UPower, WirePlumber)
**And** exactly one zbus connection is created for the system bus (NetworkManager)
**And** both connections share the single Tokio runtime (no secondary runtime)
**And** if a bus is unavailable, the connection fails silently and the dependent modules are omitted (per UX-DR16)
**And** connection handles are stored in `SharedState` and borrowed by modules

---

### Story 5.2: Battery Module — Charge Level and Charging State

As a **user**,
I want the bar to show my battery level and whether it's charging,
So that I can avoid running out of power unexpectedly.

**Acceptance Criteria:**

**Given** `battery` is in the module list and UPower is available on the session bus
**When** the bar renders
**Then** the collapsed state shows a battery icon reflecting the current charge tier (full / high / medium / low / critical)
**And** a charging indicator is shown when the battery is charging
**And** the expanded state shows icon + percentage label
**And** when battery is at or below the configured threshold (default 15%), the `CriticalBadge` overlays the icon in collapsed state showing the percentage
**And** the icon color transitions to `color.critical` at or below the critical threshold
**And** battery state updates in real time via UPower D-Bus signals (no polling)

---

### Story 5.3: Network Module — Connectivity Status

As a **user**,
I want the bar to show my current network connectivity state,
So that I can see at a glance whether I'm online and on which connection type.

**Acceptance Criteria:**

**Given** `network` is in the module list and NetworkManager is available on the system bus
**When** the bar renders
**Then** the collapsed state shows a network icon reflecting connectivity state (connected wifi / connected wired / disconnected)
**And** the expanded state shows icon + connection name (SSID for wifi, interface name for wired)
**And** the icon color transitions to `color.warning` when connectivity is limited and `color.critical` when disconnected
**And** state updates via NetworkManager D-Bus signals (no polling)
**And** if NetworkManager is unavailable, the module is silently omitted

---

### Story 5.4: Volume Module — Audio Output Level with Scroll Interaction

As a **user**,
I want the bar to show my current audio output volume and let me adjust it by scrolling,
So that I can control volume without opening a separate app.

**Acceptance Criteria:**

**Given** `volume` is in the module list and WirePlumber is available on the session bus
**When** the bar renders
**Then** the collapsed state shows a speaker icon reflecting the volume tier (high / medium / low / muted)
**And** the expanded state shows icon + percentage label
**And** scrolling up on the volume module in either collapsed or expanded state increases volume by a configurable step (default 5%)
**And** scrolling down decreases volume by the same step
**And** when muted, the icon color transitions to `color.overlay0` (dim)
**And** volume state updates via WirePlumber D-Bus signals (no polling)
**And** if WirePlumber is unavailable, the module is silently omitted

---

## Epic 6: Keyboard Navigation — A Bar Without Borders

**Goal:** The bar can be focused via a dedicated keybind, navigated with arrow keys across modules and zones, and all module interactions are reachable without a pointing device. Escape returns focus to the previous window.

**FRs covered:** FR29, FR30, FR31

**NFRs relevant:** NFR9 (fully operable without a pointing device)

**ARs relevant:** AR6 (SurfaceState owns keyboard focus state)

**UX-DRs relevant:** UX-DR3 (ModuleFrame: identical visual treatment for hover and keyboard focus), UX-DR13 (toggle model — keybind always works in both directions), UX-DR15 (dedicated keybind entry; arrow key traversal; Tab between zones; Escape exits)

---

### Story 6.1: Focus the Bar via Dedicated Keybind

As a **user who prefers keyboard-driven workflows**,
I want to press a keybind to focus the bar,
So that I can navigate and interact with it without touching the mouse.

**Acceptance Criteria:**

**Given** the bar is running and unfocused
**When** the configured keybind to focus the bar is pressed (configurable; user binds it in their compositor config)
**Then** the bar surface receives keyboard focus
**And** the first module in the top zone is highlighted with the `ModuleFrame` focus state (same rounded-rect fill as hover)
**And** focus state is stored in `SurfaceState`
**And** pressing Escape while the bar is focused returns keyboard focus to the previously focused window

---

### Story 6.2: Navigate Between Modules with Arrow Keys

As a **user with the bar focused**,
I want to move between modules using arrow keys,
So that I can reach any module without a pointer.

**Acceptance Criteria:**

**Given** the bar has keyboard focus
**When** the user presses the Up or Down arrow key
**Then** focus moves to the previous or next module within the current zone
**And** focus does not wrap within a zone (Up from first module in a zone does nothing; Down from last does nothing)
**When** the user presses Tab
**Then** focus moves to the first module in the next zone (top → center → bottom → top)
**And** Shift+Tab moves to the first module in the previous zone
**And** the focused module is visually highlighted with the `ModuleFrame` focus fill at all times

---

### Story 6.3: Activate Module Interactions via Keyboard

As a **user navigating the bar with the keyboard**,
I want to trigger module actions (click, scroll) from the keyboard,
So that I can interact with any module without switching to the mouse.

**Acceptance Criteria:**

**Given** the bar has keyboard focus and a module is highlighted
**When** the user presses Enter or Space
**Then** the module's primary action fires (equivalent to a pointer click — e.g., button executes command, workspace pill switches workspace)
**When** the user presses Up or Down arrow while focused on a scrollable module (e.g., volume)
**Then** the scroll action fires (increment/decrement)
**And** all module interactions reachable by pointer are also reachable by keyboard (NFR9)

---

### Story 6.4: Toggle Expand/Collapse via Keyboard

As a **user navigating the bar with the keyboard**,
I want to toggle the bar's expand/collapse state from the keyboard,
So that I can reveal or hide module labels without reaching for the mouse.

**Acceptance Criteria:**

**Given** the bar is in any state (focused or unfocused)
**When** the configured expand/collapse keybind is pressed
**Then** the bar toggles between collapsed and expanded states using the same animation as the pointer trigger
**And** the keybind always works in both directions (toggle model — same key inverts current state, per UX-DR13)
**And** the keybind is configurable via `bar.expand_keybind` in config
**And** the bar does not need keyboard focus for the expand/collapse keybind to function

---

## Epic 7: Multi-Output — A Bar For Every Screen

**Goal:** The bar spawns a surface on every connected output, adapts its dimensions and DPI to each screen, and supports per-output configuration overrides so users can exclude outputs or customize each bar independently.

**FRs covered:** FR3, FR4, FR5, FR35

**NFRs relevant:** NFR11 (respects compositor HiDPI/scale factor — no blurry rendering on scaled outputs)

**ARs relevant:** AR6 (SurfaceState is per-surface — each output has independent expand/focus state)

**UX-DRs relevant:** UX-DR10 (spacing system in logical pixels, scaled correctly per output)

---

### Story 7.1: Spawn a Bar Surface on Every Connected Output

As a **user with multiple monitors**,
I want a bar to appear on each of my screens,
So that I have status information and workspace navigation available on every display.

**Acceptance Criteria:**

**Given** niribar is running and multiple outputs are connected
**When** the app starts
**Then** one bar surface is created per output
**And** each surface is independently anchored to the configured edge on its respective output
**And** each surface fills the full height of its output
**And** surfaces are independent — interacting with one does not affect others
**And** each surface has its own `SurfaceState` (expand/focus state is per-surface)

---

### Story 7.2: Adapt Bar Dimensions to Output DPI and Scale Factor

As a **user with a HiDPI display**,
I want the bar to render at the correct physical size on all my screens,
So that it doesn't look blurry or incorrectly scaled on high-resolution outputs.

**Acceptance Criteria:**

**Given** an output with a scale factor greater than 1 (e.g., 2× HiDPI)
**When** the bar surface is created for that output
**Then** all dimensions (bar width, module padding, icon size) are specified in logical pixels and scaled correctly by the compositor
**And** text and SVG icons render crisply at the output's scale factor
**And** the bar width in physical pixels equals the logical width multiplied by the scale factor
**And** the bar renders correctly on outputs with fractional scale factors (e.g., 1.5×)

---

### Story 7.3: Exclude Specific Outputs via Configuration

As a **user who does not want a bar on every screen**,
I want to exclude specific outputs from having a bar,
So that my secondary monitor or projector is unobstructed.

**Acceptance Criteria:**

**Given** the config specifies `output "DP-1" { enabled = false }` (or equivalent exclusion syntax)
**When** niribar starts
**Then** no surface is created for the excluded output
**And** all other outputs continue to receive a bar surface
**And** a connected output not mentioned in config receives a bar with default settings
**And** if the excluded output is disconnected and reconnected, it remains excluded

---

### Story 7.4: Per-Output Configuration Overrides

As a **user with screens of different sizes or purposes**,
I want to override the bar configuration for specific outputs,
So that each screen's bar can have different modules, sizes, or positioning.

**Acceptance Criteria:**

**Given** the config specifies an `output "eDP-1"` block with overriding values (e.g., different module list, different collapsed width)
**When** niribar starts
**Then** the surface on that output uses the overridden values
**And** values not overridden in the output block inherit from the global config
**And** outputs without an explicit config block use the global defaults
**And** per-output config is validated at startup alongside global config — invalid overrides produce an FR37-format error

---

## Epic 8: Packaging & Distribution — A Bar For The Ecosystem

**Goal:** Deliver the three installation artifacts — systemd user service unit, Nix flake, and documented default config file — plus the CI/release pipeline, so niribar is packageable for distro repos, AUR, and Nix without special treatment.

**FRs covered:** FR40, FR41, FR42

**ARs relevant:** AR8 (GitHub Actions CI + tag-triggered release), AR9 (flake.nix + CI nix build), AR10 (contrib/systemd/niribar.service), AR11 (config/default.kdl embedded in binary)

---

### Story 8.1: Provide systemd User Service Unit File

As a **user who wants niribar to start automatically with my session**,
I want a systemd user service unit file shipped with the project,
So that I can enable it with `systemctl --user enable niribar` without writing one myself.

**Acceptance Criteria:**

**Given** the niribar repository
**When** a user installs the package
**Then** `contrib/systemd/niribar.service` is present in the repository and available as an installation artifact
**And** the unit file starts niribar as a user service after `graphical-session.target`
**And** the unit file sets `Restart=on-failure` so the bar restarts if it crashes
**And** the service description identifies the binary correctly
**And** the unit file is documented in the default config or README so users know how to enable it

---

### Story 8.2: Provide Nix Flake for Reproducible Builds

As a **NixOS or Nix user**,
I want a `flake.nix` at the repo root,
So that I can install niribar via `nix profile install` or add it to my NixOS/home-manager config without packaging it myself.

**Acceptance Criteria:**

**Given** the niribar repository root
**When** `nix build` is run
**Then** the build succeeds and produces a working niribar binary
**And** `flake.nix` and `flake.lock` are present at the repo root
**And** the flake exposes a `packages.default` output pointing to the niribar binary
**And** the flake exposes a NixOS/home-manager module (or overlay) as a convenience output
**And** CI validates `nix build` on every push to main (AR9)

---

### Story 8.3: Ship and Embed Documented Default Configuration

As a **new user installing niribar**,
I want a well-documented default config file available at install time,
So that I have a working, readable starting point without needing to write config from scratch.

**Acceptance Criteria:**

**Given** the niribar repository
**When** the binary is built
**Then** `config/default.kdl` is embedded in the binary as the fallback config (used when no user config exists)
**And** `config/default.kdl` is also present as a standalone file for users to copy and customize
**And** the default config includes the default modules (workspaces, focused-window, volume, network, battery, clock) with zone assignments matching UX-DR18
**And** every configurable field in the default config is annotated with a comment explaining its purpose and accepted values
**And** the default config is valid KDL and passes the same parser used at runtime

---

### Story 8.4: GitHub Actions CI and Release Pipeline

As a **contributor or maintainer**,
I want CI to validate every PR and produce a binary release on tagging,
So that the project stays green and releases are reproducible.

**Acceptance Criteria:**

**Given** a pull request is opened against the main branch
**When** CI runs
**Then** `cargo fmt --check` passes
**And** `cargo clippy -- -D warnings` passes
**And** `cargo test` passes
**And** `nix build` passes (on pushes to main)
**Given** a semver tag (e.g., `v1.0.0`) is pushed
**When** the release workflow runs
**Then** a binary for `x86_64-unknown-linux-gnu` is built and attached to a GitHub release

---

### Story 8.5: Verify and Document WCAG AA Accessibility Compliance

As a **user with accessibility needs**,
I want Catppuccin Mocha contrast ratios verified and documented,
So that I can trust the bar meets WCAG AA before relying on it daily.

**Acceptance Criteria:**

**Given** all color token combinations used in niribar (text on background, icon on surface, badge text on critical background, accent on surface, warning/critical icon on background)
**When** each combination is evaluated for WCAG AA contrast
**Then** all text/icon combinations meet ≥4.5:1 contrast ratio (WCAG AA small text)
**And** all UI component combinations meet ≥3:1 contrast ratio (WCAG AA non-text)
**And** results are recorded in `ACCESSIBILITY.md` at the repo root, listing each token pair, its contrast ratio, and pass/fail status
**And** any combination that fails AA is noted with a recommended token adjustment







