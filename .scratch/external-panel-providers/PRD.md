Status: ready-for-agent

# PRD: External Panel Provider Platform

## Problem Statement

Niribar's **Panels** are becoming the natural place for rich, domain-specific interactions. The original network-module idea was an interactive Wi-Fi chooser with scan results, searchable rows, credential entry, connection progress, and failure handling. Audio is likely to need a similar detailed control surface for devices, outputs, inputs, and source switching.

If each rich detail view is implemented inside Niribar core, the app will accumulate domain-specific control logic, backend dependencies, permissions, and mutable UI state for every feature. That would make **Panel Runtime**, **Module Runtime**, and app adapters harder to reason about, and it would turn Niribar into a collection of control applications instead of a status bar and panel host.

The existing **Dmenu Selection Request** path proves one useful boundary: a separate caller can ask the running Niribar instance to own the visible selector surface. However, dmenu is one-shot request/response. Wi-Fi and audio need a longer-lived bidirectional interaction where an external owner can update content as state changes and receive user interactions back.

## Solution

Add an **External Panel Provider** platform.

An **External Panel Provider** is an out-of-process owner of interactive panel content. Niribar launches it on demand from a **Configured Panel Provider**, communicates with it over framed JSON on standard input and standard output, renders its constrained declarative **External Panel Content** inside a **Panel**, and sends typed user interaction events back to the provider.

Niribar remains the owner of **Panel** placement, physical window lifecycle, styling, focus, sizing, **Selectable Search**, scrolling, and input routing. Providers own domain logic, backend integration, and state transitions. Providers update the view by sending complete replacement content models, not incremental patches.

The first version is panel-only. It supports one active **External Panel Session** at a time, following Niribar's existing one-active-**Foreground** model. It supports a small content primitive set sufficient for Wi-Fi and early audio validation: text, section/stack layout, buttons, status/error rows, **External Selectable Lists**, and **External Text Inputs** with optional secret mode.

The provider platform should be implemented before domain-specific providers. Wi-Fi should be the first validating provider after the platform exists. The built-in network **Module** remains responsible for passive status and can launch a default Wi-Fi provider when available.

## User Stories

1. As a Niribar user, I want rich module panels to look and behave like the rest of Niribar, so that detailed controls feel integrated with my bar.
2. As a Niribar user, I want clicking the network **Module** to open a Wi-Fi chooser when a provider is available, so that I can connect without leaving the bar workflow.
3. As a Niribar user, I want the network **Module** to remain useful as status-only when the Wi-Fi provider is not installed, so that missing optional helpers do not make the bar noisy.
4. As a Niribar user, I want configured module click actions to keep overriding built-in provider behavior, so that my custom configuration remains in control.
5. As a Niribar user, I want provider failures to appear as clear panel error content, so that a click never looks like it did nothing.
6. As a Niribar user, I want a provider panel to close predictably with Escape or explicit close behavior, so that it follows normal **Panel** expectations.
7. As a Niribar user, I want only one provider panel active at a time, so that panels do not stack or leave background state behind.
8. As a Niribar user, I want Wi-Fi network rows to be searchable when the Wi-Fi provider arrives, so that large scan lists are quick to navigate.
9. As a Niribar user, I want secret text entry for Wi-Fi credentials, so that provider panels can collect passwords without showing them.
10. As a Niribar maintainer, I want rich Wi-Fi control to live outside Niribar core, so that NetworkManager and iwd control logic does not expand the core app.
11. As a Niribar maintainer, I want rich audio controls to reuse the same provider platform later, so that the audio feature does not require another bespoke panel architecture.
12. As a Niribar maintainer, I want providers to be out-of-process, so that crashes and backend dependency failures do not take down Niribar.
13. As a Niribar maintainer, I want provider commands to use the existing **Configured Command** grammar, so that configuration stays consistent.
14. As a Niribar maintainer, I want provider launch to obey **Command Policy** validation, so that shell-vs-argv policy stays consistent.
15. As a Niribar maintainer, I want provider processes to have provider lifetime semantics, so that long-running interactive providers are not treated like command actions that exceeded a timeout.
16. As a Niribar maintainer, I want providers to communicate over framed JSON on standard input and standard output in v1, so that provider launch does not require service registration or socket discovery.
17. As a Niribar maintainer, I want every **External Panel Session** to start with a protocol-version handshake, so that incompatible providers fail clearly.
18. As a Niribar maintainer, I want providers to send whole content-model replacements, so that the v1 protocol avoids incremental patch complexity.
19. As a Niribar maintainer, I want Niribar to validate provider content models, so that malformed provider output cannot corrupt panel runtime state.
20. As a Niribar maintainer, I want provider UI primitives to be constrained, so that Niribar keeps ownership of styling, layout, focus, and input behavior.
21. As a Niribar maintainer, I want provider-supplied element identity to be opaque, so that Niribar does not learn provider domain semantics.
22. As a Niribar maintainer, I want Niribar to send typed events to providers, so that provider code receives stable interaction facts instead of raw widget internals.
23. As a Niribar maintainer, I want **External Selectable Lists** to be host-rendered, so that list focus, scrolling, and search behavior stay consistent with existing panels.
24. As a Niribar maintainer, I want Niribar to own **Selectable Search** state for **External Selectable Lists**, so that providers do not reimplement normal local filtering.
25. As a Niribar maintainer, I want **External Text Inputs** to support secret mode, so that Wi-Fi credentials can be collected by providers.
26. As a Niribar maintainer, I want provider content to be unable to ask Niribar to run arbitrary **Configured Commands**, so that the provider protocol does not become a command-execution API.
27. As a Niribar maintainer, I want provider failure states to be rendered inside the hosting **Panel**, so that failure handling is visible and testable.
28. As a Niribar maintainer, I want provider diagnostics to go through tracing, so that malformed output, failed launch, and timeout problems can be debugged.
29. As a Niribar maintainer, I want opening a new provider panel to close any existing **External Panel Session**, so that one-active-**Foreground** remains true.
30. As a Niribar maintainer, I want replacing a provider panel to terminate or close the old provider session, so that stale provider events cannot mutate the new panel.
31. As a Niribar maintainer, I want **Panel Runtime** to host external panel state without interpreting provider domain state, so that generic panel code stays generic.
32. As a Niribar maintainer, I want **Foreground** to remain the owner of physical panel window identity, so that provider panels follow existing replacement and stale-close rules.
33. As a Niribar maintainer, I want **Panel Actions** to support opening a named provider, so that provider panels are first-class configured interactions.
34. As a Niribar maintainer, I want **Configured Panel Providers** to be globally named, so that modules and actions can reference providers without duplicating commands.
35. As a Niribar maintainer, I want built-in modules to define optional default providers, so that useful integrations are discoverable without mandatory dependencies.
36. As a Niribar maintainer, I want user configuration to override a built-in module's default provider, so that alternative helpers can be used.
37. As a Niribar maintainer, I want user configuration to disable a built-in module's default provider, so that status-only modules remain possible.
38. As a Niribar maintainer, I want missing optional default providers to be quiet until explicitly opened, so that optional packaging does not create startup failures.
39. As a Niribar maintainer, I want missing explicitly configured providers to be surfaced on open, so that configuration mistakes are visible.
40. As a Niribar maintainer, I want provider content primitives to remain small in v1, so that the platform can be implemented and validated before becoming a full UI toolkit.
41. As a Niribar maintainer, I want Menus and Tooltips to remain out of the v1 provider platform, so that provider lifecycle stays focused on **Panels**.
42. As a Niribar maintainer, I want Wi-Fi backend details to be deferred until after the provider contract exists, so that the first provider validates the platform rather than shaping core panel code.
43. As a Niribar maintainer, I want existing **Dmenu Selection Request** behavior to remain unchanged, so that the new provider platform does not regress CLI selection workflows.
44. As a Niribar maintainer, I want existing **Panel Menu** behavior to remain unchanged, so that provider work does not rewrite launchers or menus.
45. As a Niribar maintainer, I want existing **Module Detail Content** such as clock calendar to keep working, so that external providers complement rather than replace all module detail behavior.
46. As a Niribar maintainer, I want the provider platform documented in the domain language, so that future work uses **External Panel Provider**, **External Panel Session**, and **External Panel Content** consistently.

## Implementation Decisions

- Implement the **External Panel Provider** platform before integrating Wi-Fi or audio providers.
- An **External Panel Provider** is out-of-process; Niribar does not load provider code in-process.
- Niribar launches providers on demand from a **Configured Panel Provider**.
- A **Configured Panel Provider** is globally named by **Panel Provider Name**.
- **Modules** and **Panel Actions** can reference a named **Configured Panel Provider**.
- Provider commands reuse the existing **Configured Command** grammar.
- **Command Policy** validates provider commands, including shell-form command allowance.
- Provider process lifetime is separate from command-action timeout semantics.
- A long-running provider is expected while its **External Panel Session** is active.
- Niribar communicates with launched providers using framed JSON over standard input and standard output in v1.
- The provider protocol starts with a required protocol-version handshake.
- The handshake should identify the session, relevant origin metadata, and compatible protocol version.
- A provider handshake failure, timeout, process launch failure, invalid frame, invalid content model, or unexpected provider exit renders provider error content in the hosting **Panel**.
- Provider failures are logged with diagnostics through tracing.
- Providers update **External Panel Content** by sending complete replacement content models.
- The v1 protocol does not support incremental patches.
- Provider content models must be validated before rendering.
- Provider content models are constrained declarative content, not arbitrary markup, native widgets, or scripts.
- Niribar owns rendering, styling, layout interpretation, focus, sizing, scrolling, **Selectable Search**, and input routing for **External Panel Content**.
- Providers own domain logic, backend integrations, and their own mutable domain state.
- Providers receive typed interaction events from Niribar.
- Typed provider events are addressed by opaque **External Panel Element IDs**.
- Niribar does not interpret **External Panel Element IDs** as domain meaning.
- Provider-sent content cannot ask Niribar to run arbitrary **Configured Commands**.
- Any provider-side domain work must happen inside the provider process or through future explicit host capabilities.
- The initial host action set should be minimal, such as close panel and focus-related host behavior if needed.
- V1 **External Panel Content** is panel-only.
- V1 does not support provider-owned **Menus** or provider-owned **Tooltips**.
- Niribar has at most one active **External Panel Session** at a time.
- Opening another **Panel** or **Menu** replaces the current **Foreground** and closes the active provider session if one exists.
- Opening another provider panel closes the old provider session before or as the new one becomes active.
- **Foreground** remains the owner of active interactive overlay state and physical **Panel** window identity.
- **Panel Runtime** hosts provider panel content state but does not interpret provider domain semantics.
- **Panel Sizing** should size provider content through host-known content hints and fallback panel sizing rules.
- Provider panels use existing **Panel Origin** and placement behavior.
- Add a first-class **Panel Action** for opening a provider, conceptually `open-provider`.
- `open-panel` continues to mean opening Niribar-owned panel content.
- `open-menu` continues to mean opening a named **Panel Menu**.
- A configured **Module Action** takes precedence over built-in provider activation.
- Built-in modules may provide optional default providers.
- User configuration may override or disable a built-in module's default provider.
- A missing optional built-in provider does not make the module unavailable.
- A missing explicitly configured provider is surfaced as a provider failure when the user tries to open it.
- The network **Module** remains a core passive status module.
- The network **Module** may launch a default Wi-Fi provider from built-in primary activation when no configured click action exists.
- Wi-Fi backend implementation details are deferred until after the provider contract exists.
- The already-resolved Wi-Fi product behavior should be preserved for the future provider: one row per **Wi-Fi Network**, no hidden networks in v1, open and known networks connect immediately, unknown protected networks use **Network Credential Prompt**, enterprise/EAP networks are disabled or unsupported in v1, unsupported networks stay visible, scan opens cached-first with refresh, and list ordering prioritizes active, connecting, known, supported, signal, then SSID.
- The first **External Panel Content** primitive set should remain small: text, section or stack layout, button, status/error row, **External Selectable List**, and **External Text Input**.
- **External Selectable List** entries are provider-supplied but host-rendered.
- **External Selectable Lists** support **Selectable Search** in v1.
- Search input is always visible when an **External Selectable List** list view is active.
- Niribar owns local **Selectable Search** state and matching for **External Selectable Lists**.
- Provider-filtered or remote-filtered list search is out of v1 unless later justified by a real provider.
- **External Text Input** supports normal and secret input modes.
- **External Text Input** sends value-change and submit events to the provider.
- Secret text values are provider-owned form data and are not **Selectable Search** text.
- Existing **Dmenu Selection Request**, **Panel Menu**, **Menu Search**, **Module Detail Session**, and clock detail behavior should remain compatible.
- ADR-0010 is the architecture target for external provider work.
- ADR-0009 remains the Wi-Fi backend bundling rule for the future Wi-Fi provider.

## Testing Decisions

- The confirmed testing seams are configuration, protocol, provider session, **Foreground** and **Panel Runtime**, render, module/action, and process adapter.
- Good tests should verify external behavior through stable seams, not internal collection shapes or helper function structure.
- Configuration tests should parse and validate globally named **Configured Panel Providers**.
- Configuration tests should parse and validate provider-opening **Panel Actions**.
- Configuration tests should cover provider command parsing through the existing **Configured Command** grammar.
- Configuration tests should cover shell-form provider command rejection when **Command Policy** disables shell commands.
- Configuration tests should cover module provider override and provider disable behavior.
- Configuration tests should cover invalid references to undeclared **Panel Provider Names**.
- Protocol tests should cover framed JSON request and response framing.
- Protocol tests should cover handshake success, incompatible version, missing required fields, invalid JSON, oversize frames, and malformed content models.
- Protocol tests should cover full content replacement messages.
- Protocol tests should cover typed interaction event serialization for buttons, text inputs, selectable lists, back, escape, close, and submit.
- Content-model validation tests should cover duplicate **External Panel Element IDs** where duplicates would make events ambiguous.
- Content-model validation tests should cover invalid primitive combinations, invalid list entries, invalid text input state, and invalid action references.
- Provider session tests should use a fake provider process or process adapter seam rather than relying on real external programs.
- Provider session tests should cover launch on demand, handshake timeout, provider exit, invalid provider output, and normal close cleanup.
- Provider session tests should verify provider failure content is rendered instead of silently closing the **Panel**.
- Provider session tests should verify stale provider messages after replacement are ignored.
- Provider session tests should verify whole-model replacement updates rendered content.
- **Foreground** tests should verify that only one **External Panel Session** can be active.
- **Foreground** tests should verify opening a **Menu** or normal **Panel** closes an active provider session.
- **Foreground** tests should verify replacing one provider panel with another closes the old provider session.
- **Panel Runtime** tests should verify provider panel state is hosted without interpreting provider domain meaning.
- **Panel Runtime** tests should verify Escape, Back, focus movement, and close behavior route to the right provider or host behavior.
- Render tests should verify each v1 **External Panel Content** primitive renders using Niribar-owned styling.
- Render tests should verify buttons emit typed events with opaque **External Panel Element IDs**.
- Render tests should verify **External Text Input** emits input-changed and submit events.
- Render tests should verify secret **External Text Input** does not render plain text.
- Render tests should verify **External Selectable List** focus, activation, disabled entries, active entries, and status rows.
- Render tests should verify **Selectable Search** filters and ranks **External Selectable List** entries using host-owned behavior.
- Render tests should verify search text is host state and does not require provider round trips for local filtering.
- Module/action tests should verify the network module's built-in click opens its default provider only when no configured click action exists.
- Module/action tests should verify configured click actions continue to win over built-in provider activation.
- Module/action tests should verify missing optional default providers do not make the network module unavailable.
- Module/action tests should verify explicitly configured missing providers surface provider failure on open.
- Process adapter tests should verify provider launch uses policy-validated commands but does not use command-action timeout semantics.
- Process adapter tests should verify provider stdin/stdout tasks are cleaned up when the **External Panel Session** ends.
- Existing prior art includes config parser tests, IPC protocol framed-message tests, **Foreground** replacement tests, **Panel Runtime** lifecycle tests, **Panel Menu** search tests, **Module Detail Session** tests, and app-level module action tests.
- The Wi-Fi provider itself should get its own later tests against the provider protocol after this platform exists.

## Out of Scope

- Implementing the Wi-Fi provider.
- Implementing NetworkManager or iwd scan/connect logic.
- Implementing the audio provider.
- Supporting hidden Wi-Fi networks.
- Supporting enterprise/EAP Wi-Fi credential flows.
- Adding Wi-Fi enable/disable controls.
- Adding device pickers for multiple Wi-Fi devices.
- Supporting provider-owned **Menus**.
- Supporting provider-owned **Tooltips**.
- Supporting more than one active **External Panel Session** at a time.
- Supporting persistent providers or provider registration over sockets.
- Supporting provider-filtered remote search.
- Supporting incremental content-model patches.
- Supporting arbitrary HTML, CSS, markdown, native widgets, or provider-supplied drawing commands.
- Allowing provider content to ask Niribar to run arbitrary **Configured Commands**.
- Adding in-process plugins or dynamic library loading.
- Replacing existing **Panel Menu** behavior.
- Replacing existing **Dmenu Selection Request** behavior.
- Replacing existing **Module Detail Content** for simple built-in details such as the clock calendar.
- Changing **Bar**, **Bar Surface**, **Output**, **Panel Origin**, or **Foreground** fundamentals beyond hosting external panel content.

## Further Notes

- `CONTEXT.md` has been updated with **External Panel Provider**, **External Panel Session**, **External Panel Content**, **External Panel Element ID**, **External Selectable List**, **External Text Input**, **Configured Panel Provider**, and **Panel Provider Name**.
- ADR-0010 records the core architecture decision: out-of-process providers, constrained declarative content, and stdio framed JSON for v1.
- ADR-0009 records the backend bundling rule that the future Wi-Fi provider should follow.
- The provider platform is intentionally smaller than a plugin system. The canonical term is **External Panel Provider**, not plugin.
- The main architectural goal is **Locality**: Niribar hosts interaction surfaces while providers own domain-specific control logic.
- The main caller benefit is **Leverage**: Wi-Fi, audio, and future rich controls can share the same panel host contract instead of adding custom core detail implementations.
- The deletion test for the provider platform should be strong: deleting it would remove provider launch, protocol, declarative rendering, provider event routing, failure handling, and the ability for external domain tools to reuse Niribar's panel system.
