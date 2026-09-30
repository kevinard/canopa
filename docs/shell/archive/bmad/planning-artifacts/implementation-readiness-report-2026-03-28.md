---
stepsCompleted: ['step-01-document-discovery', 'step-02-prd-analysis', 'step-03-epic-coverage-validation', 'step-04-ux-alignment', 'step-05-epic-quality-review', 'step-06-final-assessment']
documentInventory:
  prd: '_bmad-output/planning-artifacts/prd.md'
  architecture: null
  epics: null
  ux: null
---

# Implementation Readiness Assessment Report

**Date:** 2026-03-28
**Project:** niribar

## PRD Analysis

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

**Total FRs: 43**

### Non-Functional Requirements

NFR1: Expand/collapse animation renders at a consistent frame rate with no perceptible jank — GNOME Shell animations as the quality reference
NFR2: Idle CPU usage is equal to or lower than Waybar on an equivalent module configuration, as measured by published benchmarks
NFR3: Idle memory footprint is equal to or lower than Waybar on an equivalent module configuration, as measured by published benchmarks
NFR4: Niri IPC state changes (workspace transitions, focus changes) are reflected in the bar within one rendered frame — no visible delay between compositor event and bar update
NFR5: Cold start time is not perceptible during a normal Niri session startup
NFR6: D-Bus polling and event handling do not introduce render loop latency spikes — all external data sources run asynchronously
NFR7: Custom script modules (FR16) and button actions (FR28) execute with the permissions of the user running niribar — no privilege escalation, no sandboxing imposed
NFR8: niribar transmits no data over the network; all integrations are local IPC (Niri socket, D-Bus)
NFR9: The bar is fully operable without a pointing device — all module interactions reachable via keyboard (FR29-FR31)
NFR10: Default theme colors meet WCAG AA contrast ratio for text and icons against the bar background
NFR11: The bar respects the compositor's HiDPI/scale factor settings — no blurry or incorrectly sized rendering on scaled outputs

**Total NFRs: 11**

### Additional Requirements

**Domain — Wayland Protocol Compliance:**
- wlr-layer-shell required as the primary placement protocol
- Standard protocols (xdg-output, wlr-foreign-toplevel-management, ext-idle-notify) to be supported where applicable
- Niri IPC used exclusively for Niri-native modules; all other functionality uses standard protocols

**Domain — Distribution & Packaging:**
- Standard `cargo build` with no exotic build steps or vendored non-Rust dependencies
- Packageable for distro repos, AUR, Nix flake without special treatment
- Runtime dependencies minimal and clearly documented

**System Integration:**
- D-Bus as default integration mechanism; native APIs considered case-by-case
- Services: UPower (battery), NetworkManager/iwd (network), PipeWire/WirePlumber (audio), Niri IPC (workspaces/focus), systemd (autostart artifacts)

### PRD Completeness Assessment

The PRD is well-structured and information-dense. 43 FRs across 7 capability areas provide comprehensive coverage of the MVP scope. 11 NFRs cover performance, security, and accessibility with measurable criteria. Domain and packaging requirements are documented separately and complement the FRs. No obvious gaps detected at this stage — proceeding to epic coverage validation.

## Epic Coverage Validation

### Coverage Matrix

No epics document exists. All 43 FRs are uncovered.

| FR | Requirement Summary | Epic Coverage | Status |
|---|---|---|---|
| FR1 | Bar renders as persistent layer-shell surface | NOT FOUND | ❌ MISSING |
| FR2 | Bar positioned on any screen edge | NOT FOUND | ❌ MISSING |
| FR3 | Bar displayed on multiple outputs | NOT FOUND | ❌ MISSING |
| FR4 | Bar adapts to output resolution/DPI | NOT FOUND | ❌ MISSING |
| FR5 | Bar excludable from specific outputs | NOT FOUND | ❌ MISSING |
| FR6 | Bar collapses to minimal width | NOT FOUND | ❌ MISSING |
| FR7 | Bar expands to wider state | NOT FOUND | ❌ MISSING |
| FR8 | Expand/collapse transition is animated | NOT FOUND | ❌ MISSING |
| FR9 | Expand/collapse triggered by hover | NOT FOUND | ❌ MISSING |
| FR10 | Expand/collapse parameters configurable | NOT FOUND | ❌ MISSING |
| FR11 | Bar renders configurable ordered modules | NOT FOUND | ❌ MISSING |
| FR12 | Modules placeable in layout zones | NOT FOUND | ❌ MISSING |
| FR13 | Modules define collapsed/expanded states | NOT FOUND | ❌ MISSING |
| FR14 | Module data updates asynchronously | NOT FOUND | ❌ MISSING |
| FR15 | Modules respond to pointer interactions | NOT FOUND | ❌ MISSING |
| FR16 | Custom script module | NOT FOUND | ❌ MISSING |
| FR17 | Workspace module (Niri, real time) | NOT FOUND | ❌ MISSING |
| FR18 | Workspace module reflects scrollable-tiling order | NOT FOUND | ❌ MISSING |
| FR19 | Workspace module supports user labels/icons | NOT FOUND | ❌ MISSING |
| FR20 | Focused window module (optional) | NOT FOUND | ❌ MISSING |
| FR21 | Niri modules degrade gracefully elsewhere | NOT FOUND | ❌ MISSING |
| FR22 | Battery module | NOT FOUND | ❌ MISSING |
| FR23 | Network module | NOT FOUND | ❌ MISSING |
| FR24 | Volume module | NOT FOUND | ❌ MISSING |
| FR25 | Clock module | NOT FOUND | ❌ MISSING |
| FR26 | Clock module configurable format strings | NOT FOUND | ❌ MISSING |
| FR27 | Volume module responds to interaction | NOT FOUND | ❌ MISSING |
| FR28 | Button module | NOT FOUND | ❌ MISSING |
| FR29 | Bar keyboard focus and navigation | NOT FOUND | ❌ MISSING |
| FR30 | Module interactions via keyboard | NOT FOUND | ❌ MISSING |
| FR31 | Expand/collapse toggled via keyboard | NOT FOUND | ❌ MISSING |
| FR32 | Config loaded from KDL file | NOT FOUND | ❌ MISSING |
| FR33 | Default config applied when none present | NOT FOUND | ❌ MISSING |
| FR34 | Config defines modules, order, zones | NOT FOUND | ❌ MISSING |
| FR35 | Config supports per-output overrides | NOT FOUND | ❌ MISSING |
| FR36 | Config exposes expand/collapse parameters | NOT FOUND | ❌ MISSING |
| FR37 | Actionable error messages for invalid config | NOT FOUND | ❌ MISSING |
| FR38 | Version reported via CLI flag | NOT FOUND | ❌ MISSING |
| FR39 | Builds via standard cargo build | NOT FOUND | ❌ MISSING |
| FR40 | systemd user service unit file provided | NOT FOUND | ❌ MISSING |
| FR41 | Nix flake provided | NOT FOUND | ❌ MISSING |
| FR42 | Default config file provided as artifact | NOT FOUND | ❌ MISSING |
| FR43 | Functional on other wlr-layer-shell compositors | NOT FOUND | ❌ MISSING |

### Missing Requirements

All 43 FRs require epic coverage. No epics document exists yet — this is expected at the current project stage.

### Coverage Statistics

- Total PRD FRs: 43
- FRs covered in epics: 0
- Coverage percentage: 0% — **epics not yet created**

## UX Alignment Assessment

### UX Document Status

Not found.

### Alignment Issues

No UX document to validate against.

### Warnings

⚠️ **WARNING:** niribar is a user-facing graphical application with significant interaction design requirements (expand/collapse animation, keyboard navigation, per-module visual states, multi-output layout). UX documentation is strongly implied and should be created before implementation begins to ensure consistency across the bar's interaction model.

Key UX areas requiring design decisions before development:
- Expand/collapse animation design (easing, duration, trigger zones)
- Collapsed vs. expanded visual representations per module type
- Keyboard focus model (how focus moves between bar and modules)
- Default theme and visual language
- Per-output bar configuration UX

## Epic Quality Review

### Epics Assessed

No epics document found — cannot assess quality.

### Quality Findings

🔴 **Critical:** No epics exist. All 43 FRs require breakdown into epics and user stories before implementation can begin.

For reference, expected epic structure for niribar based on PRD scope:
- Epic 1 should cover foundational bar rendering and the Iced/wlr-layer-shell integration (FR1-FR5)
- Epic 2 should cover the expand/collapse interaction model (FR6-FR10)
- Epic 3 should cover the module system framework (FR11-FR16)
- Epic 4 should cover Niri-native modules (FR17-FR21)
- Epic 5 should cover system service modules (FR22-FR28)
- Epic 6 should cover keyboard/input support (FR29-FR31)
- Epic 7 should cover configuration system (FR32-FR38)
- Epic 8 should cover packaging and distribution artifacts (FR39-FR43)

Each epic must deliver independent user value and must not forward-reference future epics.

## Summary and Recommendations

### Overall Readiness Status

**NEEDS WORK** — PRD foundation is solid and complete; three prerequisite artifacts are missing before implementation can begin.

### Critical Issues Requiring Immediate Action

1. **No epics document** — All 43 FRs have zero epic coverage. Implementation cannot begin without a breakdown into epics and stories with acceptance criteria.
2. **No architecture document** — The PRD identifies two high-risk technical integration points (Iced/wlr-layer-shell, async D-Bus in Iced event loop) that must be validated architecturally before implementation. The async D-Bus prototype should be captured as an architecture decision.
3. **No UX design document** — The bar's expand/collapse interaction model, keyboard focus design, and per-module visual states need UX specification before developers can implement to a consistent standard.

### Recommended Next Steps

1. **Create Architecture** (`/bmad-create-architecture`) — Document the Iced/Wayland integration approach, async data model, module trait design, and config loading architecture. The async D-Bus + Iced event loop prototype should be validated here first.
2. **Create UX Design** (`/bmad-create-ux-design`) — Specify the expand/collapse interaction, keyboard navigation model, module visual states (collapsed/expanded), and default theme. This directly feeds the animation and rendering implementation.
3. **Create Epics & Stories** (`/bmad-create-epics-and-stories`) — Break all 43 FRs into epics and stories with acceptance criteria. Recommend 8 epics aligned to the capability areas in the PRD. Each epic must deliver independent user value.

### PRD Quality Note

The PRD is well-constructed and ready to feed downstream work:
- 43 FRs are clear, testable, and implementation-agnostic
- 11 NFRs include measurable criteria (Waybar benchmarks, GNOME Shell animation reference, WCAG AA)
- Scope is clearly bounded with explicit out-of-MVP items
- Risk mitigations are documented and actionable

No PRD revisions are required before proceeding to architecture and UX.

### Final Note

This assessment identified **3 critical gaps** across **3 artifact categories**. The PRD is the only complete artifact. All gaps are expected at this project stage — the PRD was just completed. Address the critical issues in the order listed (architecture → UX → epics) before beginning Phase 4 implementation.
