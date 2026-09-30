---
stepsCompleted: ['step-01-init', 'step-02-discovery', 'step-02b-vision', 'step-02c-executive-summary', 'step-03-success', 'step-04-journeys', 'step-05-domain', 'step-06-innovation', 'step-07-project-type', 'step-08-scoping', 'step-09-functional', 'step-10-nonfunctional', 'step-11-polish', 'step-12-complete']
inputDocuments: []
workflowType: 'prd'
briefCount: 0
researchCount: 0
brainstormingCount: 0
projectDocsCount: 0
classification:
  projectType: desktop_app
  domain: general
  complexity: medium
  projectContext: greenfield
---

# Product Requirements Document - niribar

**Author:** Kevin
**Date:** 2026-03-28

## Executive Summary

niribar is a Wayland status bar and emerging shell layer purpose-built for the Niri compositor, implemented in Rust using the Iced GUI toolkit. It addresses the gap between Niri's modern scrollable-tiling model and the available shell tooling, which remains generic and compositor-agnostic. niribar targets two audiences: power users who demand full configurability over appearance and behavior, and new Niri users who need a functional bar that works out of the box via sane defaults. The long-term vision is a modular, extensible shell that grows alongside Niri — encompassing launchers, notifications, and other desktop environment components — without compromising on resource efficiency.

**Classification:** Desktop application · Linux/Wayland-native · General domain · Medium complexity · Greenfield

### What Makes This Special

Existing bars (Waybar, eww, etc.) are portable by design, making them indifferent to any specific compositor's capabilities. niribar inverts this: Niri integration is a first-class design constraint, not an afterthought. Workspace awareness, scrollable-tiling-aware layout behavior, and a vertical+expandable panel model that aligns with Niri's horizontal workspace flow are built in, not bolted on. Iced — the GPU-accelerated Rust-native UI toolkit powering the COSMIC desktop — delivers modern visuals and smooth interactions without GTK or Qt, keeping the dependency footprint minimal and the binary lean. The result is a bar that belongs to Niri rather than one configured to tolerate it.

## Success Criteria

### User Success

- A new Niri user can install niribar, apply the default config, and have a fully functional bar that feels native to Niri — no manual tweaking required
- The expand/collapse interaction is visually smooth and responsive, meeting the standard set by GNOME Shell animations — no choppiness, no perceived lag
- A power user can configure any aspect of the bar's appearance and behavior through the config system without modifying source code
- Module extension points are documented well enough that a Rust developer can write a custom module without deep knowledge of niribar internals

### Technical Success

- Idle CPU usage and memory footprint match or beat Waybar on equivalent module configurations
- Expand/collapse animations render at a consistent frame rate with no perceptible jank — GNOME Shell as the visual quality reference
- All Niri IPC state (workspaces, focused window) is reflected in the bar accurately and in real time with no polling delay
- Cold start time is not perceptible during a normal Niri session startup

### Business Success

- niribar works well enough for daily use by its author as a full Waybar replacement on Niri
- Organic community adoption follows if quality is there — no explicit adoption targets set

### Measurable Outcomes

- Default config produces a working bar on a fresh Niri install with zero additional configuration
- Resource usage benchmarks vs. Waybar documented and maintained as part of the project
- All MVP modules implemented, tested, and stable before public announcement

## Product Scope

### MVP — Phase 1

**MVP Approach:** Experience MVP — the bar must feel complete and native to Niri from day one. Quality over speed; no deadline pressure.

**Core journeys supported:** Journey 1 (new user, zero-config), Journey 2 (power user, full config), Journey 4 (Waybar migrant, module parity)

**Must-have capabilities:**
- Vertical bar rendering via wlr-layer-shell on Niri
- Expand/collapse interaction (hover-triggered, GNOME-quality animation)
- Per-output configuration support
- KDL-based config file with sane defaults (zero-config startup)
- **Niri-native modules:** workspaces (scrollable-tiling-aware), focused window name
- **Standard modules:** clock, battery (UPower/D-Bus), network (NetworkManager/D-Bus), audio volume (PipeWire/D-Bus), clickable buttons, custom script output
- Async D-Bus integration (non-blocking render loop)
- Keyboard navigation and module interaction
- systemd user service unit file included in release artifacts
- `niribar --version` CLI flag

**Explicitly out of MVP scope:**
- Plugin/module API
- Hot-reload
- Theming system beyond basic config-driven styling
- Additional Niri-native modules (scratchpad, thumbnails, overview)
- App launcher or other shell components

### Growth — Phase 2

Features that make niribar clearly superior to Waybar for Niri users, not just different:
- Theming system: config-driven styling (colors, fonts, spacing, borders, blur)
- Hot-reload: config changes applied without restart via file watcher
- Additional Niri-native modules: scratchpad status, window thumbnails, overview integration
- First-party plugin/module API: defined Rust trait for community modules
- Migration guide: Waybar → niribar config translation reference

### Vision — Phase 3

niribar as the shell layer for Niri:
- **App launcher** — first non-bar component; leverages the expand model natively
- Notification daemon
- Additional shell components as the Niri ecosystem matures
- Potential "Niri shell" meta-project packaging

### Risk Mitigation

**Iced viability:** Iced is pre-1.0 and API-unstable. System76's COSMIC Desktop is a production DE built entirely on Iced, providing institutional backing. Iced's wgpu GPU backend provides more direct rendering control than GTK/Cairo. *Mitigation:* Pin Iced version per release; monitor COSMIC upstream as a bellwether; document upgrade path in CHANGELOG.

**Async D-Bus in Iced event loop:** The interaction between Iced's event loop and async D-Bus subscriptions is the most uncertain MVP integration point. *Mitigation:* Prototype this integration before building any modules; use `zbus` (async-native Rust D-Bus library) as the D-Bus layer.

**Niri ecosystem size:** The compositor-first bet pays off only if Niri adoption continues. *Mitigation:* Correct wlr-layer-shell implementation ensures niribar runs on Hyprland, sway, and others as a fallback audience.

**Iced API/protocol version drift:** Both Iced and Wayland protocols evolve. *Mitigation:* Pin and document protocol and Iced versions per release; separate Niri IPC code cleanly from standard Wayland code in architecture.

## User Journeys

### Journey 1: The New Niri User — "It Just Works"

**Meet Alex.** Alex has been on Hyprland for a year but keeps hearing about Niri's scrollable-tiling model. After a weekend of distro-hopping, they've landed on a fresh Niri install. The compositor feels great — but the screen is bare. They need a bar.

**Opening Scene:** Alex searches "Niri bar" and finds niribar in the Niri wiki. The README promises a default config that works out of the box. Skeptical but hopeful — their last bar setup took three hours — they install it.

**Rising Action:** They run niribar with no config file. A vertical bar appears on the left edge: workspaces showing their current Niri layout, a clock, battery, network, and audio — all working. Alex hovers the bar; it expands to reveal labels. They pull their hand back; it collapses. No configuration was touched.

**Climax:** Alex opens several windows across workspaces. The workspace module updates in real time, reflecting Niri's scrollable layout exactly. Focused window title changes as they switch. The bar *knows* Niri — not like a generic bar bolted on.

**Resolution:** Alex's Niri setup is complete in under ten minutes. niribar is running, and it feels native.

**Capabilities revealed:** Default config, zero-config startup, Niri workspace module, focused window module, expand/collapse on hover, clock/battery/network/audio modules.

---

### Journey 2: The Power User — "Every Pixel Mine"

**Meet Soren.** Soren has been ricing Linux desktops for six years. Their current Waybar config is 400 lines of JSON and CSS. They've switched to Niri and want a bar that actually understands Niri's workspace model.

**Opening Scene:** Soren reads the niribar config documentation before installing. They want to know: can they do everything they do in Waybar, plus more?

**Rising Action:** They start with the default config and systematically replace each section — rearranging modules, adjusting spacing, setting custom colors. They configure the expand width, animation easing, and hover trigger zone. For one module they need behavior not built-in, so they write a custom script module.

**Climax:** Soren configures the workspace module to display Niri workspace names with custom icons per workspace. The bar now reflects their Niri session exactly — something Waybar could never do cleanly.

**Resolution:** Soren's config is 200 lines — half of their old Waybar config — and does more. They post a screenshot. Someone asks for their dotfiles.

**Capabilities revealed:** Full config system, theming (colors, fonts, spacing, animation parameters), custom script modules, workspace label/icon formatting, fine-grained layout control.

---

### Journey 3: The Module Developer — "I Need This Module"

**Meet Priya.** Priya uses niribar daily but needs a module displaying her Tailscale VPN status with a click-to-toggle action. It doesn't exist yet. She's comfortable with Rust.

**Opening Scene:** Priya reads the module development guide: what interface does she implement, what data can she access, how does she wire click handlers?

**Rising Action:** She clones the repo, finds the module trait, reads the example module. The API is clear — implement a data-fetching function, a render function, and optionally an interaction handler. She writes her Tailscale module in an afternoon.

**Climax:** The module appears in the bar, updates on network change, and toggles correctly on click. She opens a PR; the maintainer accepts it.

**Resolution:** Priya's module ships in the next release. Three other users immediately use it. The plugin surface has proven itself.

**Capabilities revealed:** Module API (trait/interface), click/interaction handlers, data polling abstraction, module documentation, example module, contribution pathway.

---

### Journey 4: The Waybar Migrator — "Don't Make Me Start Over"

**Meet Dmitri.** Dmitri has a finely tuned Waybar setup on Niri — it mostly works, but the workspace module doesn't understand Niri's scrollable model and occasionally desynchs.

**Opening Scene:** Dmitri compares the niribar KDL config format to his Waybar JSON. The structure differs but concepts map: modules, positions, styling. He reads the migration guide.

**Rising Action:** Clock, battery, network — direct equivalents exist. Custom script modules port over with minor syntax changes. CSS styles translate to config values.

**Climax:** The workspace module shows his actual Niri workspace state — correct names, correct active indicator, scrollable-layout-aware ordering. It just works.

**Resolution:** Dmitri's migration takes a few hours. His new config is cleaner. He removes Waybar.

**Capabilities revealed:** Config documentation, migration guide, module parity with common Waybar modules, theming system expressiveness, workspace module correctness.

---

### Journey Requirements Summary

| Capability Area | Revealed By |
|---|---|
| Zero-config default startup | Journey 1 |
| Niri workspace + focused window modules | Journeys 1, 4 |
| Expand/collapse on hover | Journey 1 |
| Standard modules (clock, battery, network, audio) | Journeys 1, 4 |
| Full theming system | Journey 2 |
| Custom script modules | Journeys 2, 4 |
| Fine-grained config (animation, layout, spacing) | Journey 2 |
| Module API / plugin interface | Journey 3 |
| Click/interaction handlers in modules | Journey 3 |
| Migration documentation | Journey 4 |
| Module development guide + example | Journey 3 |

## Domain-Specific Requirements

### Wayland Protocol Compliance

- niribar must implement standard Wayland protocols correctly and fully, not only the subset required for Niri
- **wlr-layer-shell** is the primary protocol for bar/panel placement and must be supported as the foundation for rendering and positioning
- Other relevant protocols (e.g., xdg-output, wlr-foreign-toplevel-management, ext-idle-notify) must be supported where applicable to bar functionality
- Niri IPC is used exclusively for Niri-native modules; all other functionality uses standard protocols
- Correct protocol implementation makes niribar runnable on other wlr-layer-shell-compatible compositors (Hyprland, sway, etc.) minus Niri-specific modules, without explicit porting effort

### Distribution & Packaging

- Builds via standard `cargo build` — no exotic build steps, patching, or vendored non-Rust dependencies
- Runtime dependencies minimal and clearly documented — no implicit system library requirements beyond what Iced and the Wayland stack require
- Packageable for major Linux distribution formats (distro repos, AUR, Nix flake) without special treatment
- Nix flake provided as a first-class packaging path, given Niri user base overlap with NixOS

## Innovation & Novel Patterns

### Detected Innovation Areas

**1. The Expandable Vertical Bar as a Shell Primitive**

niribar reframes the status bar as a collapsible panel — not a strip of status indicators, but a surface that expands to reveal richer content and functionality. This interaction model aligns naturally with Niri's horizontal scrollable-tiling layout and generalizes: the same expand/collapse mechanism that reveals module labels in the bar can host a full app launcher. Most bars are static horizontal strips with no expand model at all.

**2. Compositor-First Architecture**

Every major Wayland bar (Waybar, eww, Yambar) is compositor-agnostic by design. niribar inverts this: deep Niri integration is a first-class design constraint. Workspace awareness, scrollable-tiling-aware layout behavior, and Niri IPC are built into the core, not shimmed on top. This constrains the user base by design in exchange for a qualitatively better experience for Niri users.

**3. Iced as the Bar Rendering Stack**

Using a GPU-accelerated, Rust-native UI toolkit for a status bar is unusual. Most bars use GTK, Qt, or web-based rendering. Iced is proven at the DE level by COSMIC; niribar applies it at the component level. This yields modern visuals and smooth animations without heavyweight runtime dependencies, and positions niribar on a technically coherent path toward a full shell layer.

### Market Context & Competitive Landscape

- **Waybar**: Dominant Wayland bar. GTK-based, compositor-agnostic. Has Niri module support but no native Niri IPC integration. Horizontal-first.
- **eww**: Declarative Rust widget system. Flexible but high setup effort; not bar-first.
- **Yambar**: Modular, lightweight, protocol-compliant. No Niri-specific integration.
- **Gap**: No bar in the ecosystem is designed around Niri's compositor model or offers a vertical expand/collapse interaction primitive.

### Validation Approach

- **Expand/collapse model:** Validated through daily use — does the interaction feel natural switching between collapsed (minimal footprint) and expanded (full labels/features)?
- **Compositor-first bet:** Validated if Niri workspace state is always accurate across Niri's edge cases (empty workspaces, named workspaces, focus transitions)
- **Iced rendering:** Validated against resource benchmarks vs. Waybar and subjective animation quality vs. GNOME Shell reference

## Desktop Application Specific Requirements

### Platform Support

- **Target platform:** Linux, Wayland only
- **Compositor:** Niri (primary); other wlr-layer-shell-compatible compositors supported as a side effect of correct protocol implementation
- **Display protocol:** Wayland exclusively — no X11/XWayland support planned
- **Architecture:** x86_64 primary; aarch64 considered if build infrastructure allows
- **Minimum Wayland protocol versions:** documented per protocol during implementation

### System Integration

D-Bus is the default integration mechanism; native APIs considered for specific integrations where they provide a clear advantage.

| Service | Integration Method | Used By |
|---|---|---|
| Niri compositor | Niri IPC (Unix socket) | Workspace module, focused window module |
| UPower | D-Bus | Battery module |
| NetworkManager / iwd | D-Bus | Network module |
| PipeWire / WirePlumber | D-Bus (default); native bindings considered | Audio/volume module |
| systemd user session | Unit files provided | Autostart (optional) |

niribar provides a `systemd --user` service unit file as an installation artifact. Users may start niribar via Niri's `spawn-at-startup`, systemd, or any other method — no autostart behavior is enforced.

### Multi-Output Support

- Each connected output is independently configurable
- Users can specify which outputs show the bar, which default to collapsed, and which are excluded
- Per-output config supports different module sets, widths, and expand behavior
- Bar position and size adapt to each output's resolution and scale factor

### Update Strategy

- Updates fully delegated to the system package manager — no in-app update mechanism, auto-updater, or telemetry
- Version exposed via `niribar --version`
- Release artifacts: compiled binary, systemd unit files, default config file, Nix flake

## Functional Requirements

### Bar Rendering & Layout

- **FR1:** The bar renders as a persistent layer-shell surface on a Wayland compositor
- **FR2:** The bar can be positioned on any screen edge, with vertical side placement as the primary use case
- **FR3:** The bar can be displayed on multiple outputs simultaneously
- **FR4:** The bar adapts its size and positioning to each output's resolution and DPI scale factor
- **FR5:** The bar can be excluded from specific outputs via configuration

### Expand/Collapse Interaction

- **FR6:** The bar collapses to a minimal width displaying only compact/icon content
- **FR7:** The bar expands to a wider state revealing labels and additional content
- **FR8:** The transition between collapsed and expanded states is animated
- **FR9:** Expand/collapse can be triggered by pointer hover
- **FR10:** Expand/collapse trigger, animation, and width targets are configurable

### Module System

- **FR11:** The bar renders a configurable ordered set of modules
- **FR12:** Modules can be placed in distinct layout zones (top, center, bottom of bar)
- **FR13:** Each module can define distinct visual representations for collapsed and expanded states
- **FR14:** Module data sources update asynchronously without blocking bar rendering
- **FR15:** Modules can respond to user pointer interactions (click, scroll)
- **FR16:** A custom script module executes a shell command and displays its output in the bar

### Niri Integration

- **FR17:** A workspace module displays the current state of all Niri workspaces in real time
- **FR18:** The workspace module reflects Niri's scrollable-tiling workspace ordering and structure
- **FR19:** The workspace module supports user-defined labels and icons per workspace
- **FR20:** A focused window module displays the title of the currently focused window in real time; it is an optional module, not a core bar component
- **FR21:** Niri-specific modules are isolated and degrade gracefully on non-Niri compositors

### System Service Modules

- **FR22:** A battery module displays current battery charge level and charging state
- **FR23:** A network module displays current network connectivity status
- **FR24:** A volume module displays current audio output volume level
- **FR25:** A clock module displays the current date and time
- **FR26:** The clock module supports configurable format strings for date and time display
- **FR27:** The volume module responds to user interaction to adjust output volume
- **FR28:** A button module executes a user-defined action when activated

### Keyboard & Input

- **FR29:** The bar can be focused and navigated using keyboard input
- **FR30:** Module interactions (activation, scroll actions) can be triggered via keyboard when the bar is focused
- **FR31:** The expand/collapse state can be toggled via keyboard

### Configuration

- **FR32:** The bar loads configuration from a KDL file at startup
- **FR33:** A complete default configuration is applied when no user config file is present
- **FR34:** The configuration defines which modules are active, their order, and their layout zone placement
- **FR35:** The configuration supports per-output overrides for module sets and bar behavior
- **FR36:** The configuration exposes expand/collapse parameters including trigger mode, animation, and width
- **FR37:** The bar reports actionable error messages when the configuration file is invalid
- **FR38:** The bar reports its version number via a CLI flag

### Packaging & Distribution

- **FR39:** The project builds via standard `cargo build` with no non-Rust build steps or exotic tooling
- **FR40:** The project provides a systemd user service unit file as an installation artifact
- **FR41:** The project provides a Nix flake as a first-class packaging path
- **FR42:** The project provides a documented default configuration file as an installation artifact
- **FR43:** The bar is functional on wlr-layer-shell-compatible compositors beyond Niri, with Niri-specific modules inactive

## Non-Functional Requirements

### Performance

- **NFR1:** Expand/collapse animation renders at a consistent frame rate with no perceptible jank — GNOME Shell animations as the quality reference
- **NFR2:** Idle CPU usage is equal to or lower than Waybar on an equivalent module configuration, as measured by published benchmarks
- **NFR3:** Idle memory footprint is equal to or lower than Waybar on an equivalent module configuration, as measured by published benchmarks
- **NFR4:** Niri IPC state changes (workspace transitions, focus changes) are reflected in the bar within one rendered frame — no visible delay between compositor event and bar update
- **NFR5:** Cold start time is not perceptible during a normal Niri session startup
- **NFR6:** D-Bus polling and event handling do not introduce render loop latency spikes — all external data sources run asynchronously

### Security

- **NFR7:** Custom script modules (FR16) and button actions (FR28) execute with the permissions of the user running niribar — no privilege escalation, no sandboxing imposed; users are responsible for configured scripts
- **NFR8:** niribar transmits no data over the network; all integrations are local IPC (Niri socket, D-Bus)

### Accessibility

- **NFR9:** The bar is fully operable without a pointing device — all module interactions reachable via keyboard (FR29-FR31)
- **NFR10:** Default theme colors meet WCAG AA contrast ratio for text and icons against the bar background
- **NFR11:** The bar respects the compositor's HiDPI/scale factor settings — no blurry or incorrectly sized rendering on scaled outputs
