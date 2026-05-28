Status: ready-for-agent

# PRD: Bar Surface Registry Reconciliation

## Problem Statement

Niribar's **Bar Surface** lifecycle is spread across startup, output hotplug, panel placement, and IPC-triggered panel toggling. The same policy appears in several places: selecting which **Bars** match an **Output**, ordering horizontal **Bar Surfaces** before vertical **Bar Surfaces**, storing live identity as loose `output_name` and `bar_name` strings, and reconstructing **Panel Origin** placement data from `SurfaceState`.

From a maintainer's perspective, this makes **Bar Surface** behavior harder to change safely. A small lifecycle change can require editing boot logic, hotplug logic, global toggle selection, overlay cleanup, and **Panel** placement code. The current shape also hides a real domain concept, **Bar Surface Identity**, behind repeated string tuples and empty-string fallback conventions.

## Solution

Deepen the **Bar Surface Registry** so it owns reconciliation between configured **Bars**, current **Eligible Outputs**, and live **Bar Surfaces**.

The registry should treat **Bar Surface Identity** as a first-class value. A **Bar Surface Identity** identifies one desired or live **Bar Surface** independently of its layer-shell window ID. In AllScreens mode, the identity uses the **Implicit Bar** diagnostic **Bar Name** `main` and no specific **Output**.

The registry should calculate desired **Bar Surfaces**, compare them to live **Bar Surfaces**, and emit lifecycle effects for the layer-shell adapter to execute. It should own deterministic selection for **Global Panel Toggle**, keep enough origin metadata for **Panel** placement, and clean up overlays associated with a disappearing **Bar Surface**.

## User Stories

1. As a Niribar maintainer, I want **Bar Surface** identity represented explicitly, so that lifecycle code does not depend on tuple ordering or empty-string conventions.
2. As a Niribar maintainer, I want the **Bar Surface Registry** to calculate desired **Bar Surfaces** from **Eligible Outputs** and configured **Bars**, so that startup and hotplug use the same policy.
3. As a Niribar maintainer, I want the registry to reconcile desired and live **Bar Surfaces**, so that missing surfaces open and no-longer-desired surfaces close through one module.
4. As a Niribar maintainer, I want horizontal **Bar Surfaces** opened before vertical **Bar Surfaces**, so that the compositor can allocate remaining **Edge Span** correctly.
5. As a Niribar maintainer, I want the same ordering rule used during boot and output changes, so that hotplug behavior matches startup behavior.
6. As a Niribar maintainer, I want AllScreens mode to use the **Implicit Bar** diagnostic **Bar Name** `main`, so that the **Bar Name** remains meaningful even without a specific **Output**.
7. As a Niribar user, I want existing **Bar Surfaces** to remain open when their **Output** is still eligible, so that output changes do not unnecessarily recreate stable bars.
8. As a Niribar user, I want **Bar Surfaces** on no-longer-eligible **Outputs** to close, so that stale bars do not remain after an output disappears or stops matching.
9. As a Niribar user, I want **Panels**, **Menus**, and **Tooltips** associated with a disappearing **Bar Surface** to close with it, so that overlays do not outlive their origin.
10. As a Niribar user, I want **Global Panel Toggle** to choose a deterministic **Bar Surface**, so that IPC-triggered panel behavior is predictable.
11. As a Niribar user, I want **Global Panel Toggle** to prefer the focused **Output** when one is known, so that the default **Panel** appears where I am working.
12. As a Niribar user, I want **Global Panel Toggle** to fall back to one deterministic **Bar Surface** when no focused **Output** is known, so that it never opens multiple panels.
13. As a Niribar user, I want **Panel** placement to use the originating **Bar Surface**'s **Bar**, **Output**, screen size, and origin data, so that panels open beside the correct **Edge**.
14. As a Niribar maintainer, I want **Panel Origin** lookup centralized in the registry, so that panel placement does not reconstruct surface facts from loose state fields.
15. As a Niribar maintainer, I want layer-shell task construction kept outside the registry, so that the registry stays focused on lifecycle policy rather than iced task execution.
16. As a Niribar maintainer, I want the registry to emit lifecycle effects, so that tests can verify registry decisions without opening real windows.
17. As a Niribar maintainer, I want current Niri-derived output names adapted into **Eligible Outputs**, so that **Niri Integration** does not become the **Bar Surface** lifecycle seam.
18. As a Niribar maintainer, I want the design to respect named **Bars** and **Output Targets**, so that ADR-0001 remains the lifecycle model.
19. As a Niribar maintainer, I want **Bar Surface** creation to remain independent of **Niri Integration** availability, so that ADR-0002 remains intact.
20. As a Niribar maintainer, I want registry tests to cover startup, hotplug, output removal, AllScreens mode, and global toggle selection, so that future lifecycle changes are localized.

## Implementation Decisions

- Build or deepen the **Bar Surface Registry** as the module that owns desired/live **Bar Surface** reconciliation.
- Add **Bar Surface Identity** as a first-class domain value.
- A **Bar Surface Identity** contains a **Bar Name** and an optional **Output**.
- AllScreens mode uses **Bar Name** `main` and no specific **Output** instead of empty strings as a public registry convention.
- The registry consumes **Eligible Outputs** as input from a higher layer.
- The current Niri workspace-derived output list may remain the initial adapter that feeds **Eligible Outputs** into the registry.
- **Niri Integration** must not become the lifecycle seam for **Bar Surface** creation.
- The registry compares desired identities with live identities and emits lifecycle effects.
- Lifecycle effects include opening missing **Bar Surfaces**, closing no-longer-desired **Bar Surfaces**, and keeping existing **Bar Surfaces**.
- Open effects preserve the existing rule that horizontal **Bar Surfaces** are opened before vertical **Bar Surfaces** for each **Output**.
- Close effects include cleanup of overlays associated with the closing **Bar Surface**.
- The registry owns the mapping from **Bar Surface Identity** to layer-shell window ID and from window ID back to identity.
- The registry owns deterministic selection for **Global Panel Toggle**.
- The registry owns lookup data needed to resolve a **Panel Origin** into its originating **Bar**, **Output**, window ID, screen size, and surface origin inputs.
- Layer-shell task creation remains outside the registry, in an adapter or existing `App` orchestration code.
- The registry interface should expose lifecycle intent and lookup behavior, not iced task construction.
- Existing **Bar Surface Registry** overlay responsibilities remain in scope because disappearing **Bar Surfaces** must close associated **Panels**, **Menus**, and **Tooltips**.
- The refactor should be delivered in vertical slices: identity and read-only queries first, then creation reconciliation, then close reconciliation and overlay cleanup.

## Testing Decisions

- Tests should verify external registry behavior through its interface: desired/live reconciliation, emitted lifecycle effects, ordering, and lookup results.
- Tests should avoid asserting implementation details such as internal collection shapes unless those shapes are part of the registry interface.
- The **Bar Surface Registry** should have focused unit tests for identity creation, desired-set calculation, open ordering, close detection, and deterministic **Global Panel Toggle** selection.
- Registry tests should cover AllScreens mode and confirm it uses the **Implicit Bar** diagnostic **Bar Name** `main` with no specific **Output**.
- Registry tests should cover multiple **Bars** matching one **Output**, including additive **Output Targets**.
- Registry tests should cover output removal and verify associated overlay close effects are emitted.
- Existing tests around surface lifecycle, panel placement, global toggle behavior, and overlay cleanup provide prior art and should be migrated or supplemented as call sites move.
- App-level tests should remain for integration behavior where iced messages and layer-shell tasks are relevant, but policy tests should move toward the registry interface.

## Out of Scope

- Adding third-party **Bar** or **Module** plugins.
- Changing **Output Target** configuration syntax or semantics.
- Changing ADR-0001's named **Bar** model.
- Changing ADR-0002's **Niri Integration** role.
- Reworking **Panel Runtime** interaction state beyond the cleanup and lookup needed for **Bar Surface** lifecycle.
- Replacing the layer-shell adapter or changing iced task execution semantics.
- Automatically resolving overlap between **Bar Surfaces** that share the same **Output** and **Edge**.

## Further Notes

- The glossary now includes **Bar Surface Identity**.
- The main architectural goal is **Locality**: lifecycle policy should stop leaking into boot, hotplug, panel placement, and global toggle code.
- The registry should gain **Depth** by putting substantial behavior behind a small, testable interface while leaving layer-shell execution as an adapter concern.
