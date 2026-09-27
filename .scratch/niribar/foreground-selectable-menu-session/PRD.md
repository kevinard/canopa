Status: ready-for-agent

# PRD: Foreground and Selectable Menu Session

## Problem Statement

Niribar currently handles **Bar Surfaces**, interactive **Menus**, **Panels**, **Panel Menus**, **Dmenu Selection Requests**, and **Tooltips** through several sibling runtime modules and helper modules coordinated by the app root. The concepts share replacement, focus, hover, search, selection, physical window, and stale-close behavior, but those responsibilities are implemented multiple times across the current architecture.

From a maintainer's perspective, this makes the app hard to reason about. Adding **Menu Search** to **SNI Menus** would deepen the duplication because **Panel Menus** already have search, focus, nested entry behavior, and activation completion, while **SNI Menus** separately own hover, focus, popup windows, DBusMenu activation, and layout updates.

The larger architectural problem is that Niribar does not have one owner for the currently active interactive overlay. Previous architecture split logical **Panel Lifecycle** from overlay-window lifecycle, but that split still leaves **Panel**, **Menu**, and selectable-list behavior coordinated indirectly. The app needs a deeper module that models interactive overlay state directly.

## Solution

Introduce **Foreground** as the deep module for interactive overlay state. **Foreground** represents the currently active interactive overlay: no overlay, a **Menu**, or a **Panel**. It owns the physical overlay window identity for interactive overlays, the active interaction session, replacement behavior, stale close handling, and open/toggle/close transitions.

Keep **Tooltip** outside **Foreground**. A **Tooltip** is passive, non-interactive, and remains owned as a **Bar Surface** affordance.

Introduce **Selectable Menu Session** as the shared interaction module for selectable **Menus** and **Panel Menus**. It owns active menu level, pointer hover, keyboard focus, optional **Menu Search**, nested flattening with breadcrumb context, optional **Custom Menu Value**, and selected-entry completion. It returns a generic selected entry. The owning **Foreground** adapter interprets that selected entry as a **Panel Action**, **Installed Desktop Application** launch, **Dmenu Selection Request** response, or **SNI Menu** item activation.

Add configurable **Menu Search** for **SNI Menus** through the tray **Module**. **SNI Menus** do not show **Menu Search** by default. When enabled, they render a visible search input while open and use the same matching, ranking, nested-entry, and breadcrumb behavior as **Panel Menus** and **Dmenu Selection Requests**.

This PRD follows ADR-0008 and supersedes ADR-0007 for future architecture work.

## User Stories

1. As a Niribar maintainer, I want one **Foreground** state for interactive overlays, so that **Menu** and **Panel** replacement behavior has one owner.
2. As a Niribar maintainer, I want **Foreground** to represent no overlay, a **Menu**, or a **Panel**, so that the app's active interactive state is explicit.
3. As a Niribar maintainer, I want **Tooltip** to remain outside **Foreground**, so that passive hover affordances are not forced into the interactive overlay model.
4. As a Niribar maintainer, I want **Foreground** to own physical **Menu** and **Panel** window identity, so that stale close handling is local to the active interactive overlay.
5. As a Niribar maintainer, I want every request that opens, toggles, or closes a **Panel** or **Menu** to go through **Foreground**, so that alternate paths do not reintroduce coordination bugs.
6. As a Niribar maintainer, I want configured **Panel Actions** to open and toggle through **Foreground**, so that configured interactions use the same lifecycle as runtime interactions.
7. As a Niribar maintainer, I want tray **Menu** requests to open through **Foreground**, so that **SNI Menu** lifecycle matches **Panel** lifecycle.
8. As a Niribar maintainer, I want keyboard Escape to close through **Foreground**, so that close behavior is consistent for **Menus** and **Panels**.
9. As a Niribar maintainer, I want focus loss to close or preserve interactive overlays through **Foreground**, so that focus behavior is not split across role-specific handlers.
10. As a Niribar maintainer, I want IPC-triggered **Global Panel Toggle** to route through **Foreground**, so that IPC and pointer interactions cannot diverge.
11. As a Niribar user, I want opening a **Menu** to close the current **Panel**, so that the interface has one active interactive overlay.
12. As a Niribar user, I want opening a **Panel** to close the current **Menu**, so that interactions never stack unpredictably.
13. As a Niribar user, I want opening a **Menu** to close open **Tooltips** on the same **Bar Surface**, so that passive hover information does not obscure the interactive overlay.
14. As a Niribar user, I want a **Tooltip** to remain a passive **Bar Surface** affordance, so that hover behavior stays lightweight.
15. As a Niribar maintainer, I want **Bar Surface Registry** to narrow back toward live **Bar Surface** records and passive **Tooltip** metadata, so that it does not also own interactive overlay lifecycle.
16. As a Niribar maintainer, I want **Panel Runtime** behavior to move into **Foreground** where it concerns interactive overlay lifecycle, so that **Panel** and **Menu** lifecycles are not sibling systems coordinated by the app root.
17. As a Niribar maintainer, I want **Panel** content behavior to remain distinguishable from **Panel** window lifecycle, so that module detail content and menu content can evolve without duplicating window code.
18. As a Niribar maintainer, I want **Panel Menu** content to use **Selectable Menu Session**, so that **Panel Menu** focus, search, back, and activation are not panel-specific implementations.
19. As a Niribar maintainer, I want **SNI Menus** to use **Selectable Menu Session**, so that **SNI Menu** hover and keyboard focus do not live in separate maps.
20. As a Niribar maintainer, I want **Dmenu Selection Requests** to use **Selectable Menu Session**, so that selection behavior is shared with other searchable menus.
21. As a Niribar maintainer, I want **Selectable Menu Session** to own pointer hover, so that pointer and keyboard highlight behavior have one state model.
22. As a Niribar maintainer, I want **Selectable Menu Session** to own keyboard focus, so that navigation is independent of physical presentation.
23. As a Niribar maintainer, I want **Selectable Menu Session** to own active menu level, so that nested menu traversal has one implementation.
24. As a Niribar maintainer, I want **Selectable Menu Session** to own **Menu Search** text, so that search state is not panel-specific.
25. As a Niribar maintainer, I want **Selectable Menu Session** to own nested flattening with breadcrumb context, so that searchable nested menus behave consistently.
26. As a Niribar maintainer, I want **Selectable Menu Session** to own **Custom Menu Value** behavior, so that dmenu custom values are not a separate branch outside the menu model.
27. As a Niribar maintainer, I want **Selectable Menu Session** to return generic selected entries, so that it does not know about **Panel Actions**, DBusMenu, IPC, or process launch.
28. As a Niribar maintainer, I want **Panel Menu** adapters to interpret generic selected entries as **Panel Actions** or **Installed Desktop Application** launches, so that effect ownership remains outside the session.
29. As a Niribar maintainer, I want **Dmenu Selection Request** adapters to interpret generic selected entries as IPC responses, so that the session does not know about responders.
30. As a Niribar maintainer, I want **SNI Menu** adapters to interpret generic selected entries as DBusMenu item activation, so that DBus-specific behavior stays out of the session.
31. As a Niribar user, I want existing **Panel Menu Search** behavior to continue working, so that configured launchers remain searchable.
32. As a Niribar user, I want installed-application **Panel Menus** to remain searchable, so that application launching does not regress.
33. As a Niribar user, I want **Dmenu Selection Mode** search to keep its existing behavior, so that CLI workflows are preserved.
34. As a Niribar user, I want **Dmenu Selection Mode** to allow **Custom Menu Values** by default, so that arbitrary typed submissions still work.
35. As a Niribar user, I want `--no-custom` behavior to remain available for **Dmenu Selection Mode**, so that callers can restrict selection to provided entries.
36. As a Niribar user, I want **SNI Menus** to have no visible search input by default, so that tray context menus remain compact unless configured otherwise.
37. As a Niribar user, I want to configure **Menu Search** for **SNI Menus**, so that large tray context menus can be searched.
38. As a Niribar user, I want a searchable **SNI Menu** to show a visible search input, so that the search affordance is discoverable.
39. As a Niribar user, I want searchable **SNI Menus** to use the same fuzzy matching as **Panel Menus**, so that search behavior is predictable.
40. As a Niribar user, I want searchable **SNI Menus** to search nested entries with breadcrumb context, so that deeply nested tray actions are reachable quickly.
41. As a Niribar user, I want searchable **SNI Menus** to preserve source order when search text is blank, so that unsearched menus behave like normal menus.
42. As a Niribar user, I want a **SNI Menu** layout update to reset the menu to top level with empty **Menu Search** text and initial focus, so that stale DBusMenu item state does not survive source replacement.
43. As a Niribar maintainer, I want **Menu Search** matching rules to be shared everywhere search is enabled, so that future ranking changes happen in one module.
44. As a Niribar maintainer, I want **Menu Search** to remain Niribar-owned behavior, so that the domain does not expose a third-party matcher as public behavior.
45. As a Niribar maintainer, I want selectable menu snapshots to drive rendering, so that physical **Menu** and **Panel** views do not inspect session internals.
46. As a Niribar maintainer, I want **Foreground** snapshots to drive view classification, so that view selection does not probe unrelated popup maps.
47. As a Niribar maintainer, I want stale **Menu** close events to be ignored when they do not match current **Foreground**, so that old windows cannot close new overlays.
48. As a Niribar maintainer, I want stale **Panel** close events to be ignored when they do not match current **Foreground**, so that replacement remains safe.
49. As a Niribar maintainer, I want replacing a **Panel** with another **Panel** to make the old **Panel** logically gone immediately, so that old input cannot mutate the new foreground.
50. As a Niribar maintainer, I want replacing a **Menu** with another **Menu** to make the old **Menu** logically gone immediately, so that old input cannot mutate the new foreground.
51. As a Niribar maintainer, I want searchable **SNI Menu** tests to exercise the same search seam as **Panel Menu** tests, so that shared behavior is protected once.
52. As a Niribar maintainer, I want **Foreground** tests to cover replacement and stale close behavior, so that the old **Panel Attachment** coordination bug does not return.
53. As a Niribar maintainer, I want **Bar Surface Registry** tests to stop asserting ownership of interactive **Menu** and **Panel** metadata after migration, so that tests do not preserve the old architecture.
54. As a Niribar maintainer, I want **Tooltip** tests to continue proving passive hover behavior, so that removing tooltips from **Foreground** does not weaken tooltip guarantees.
55. As a Niribar maintainer, I want a tracer-bullet migration path, so that the architecture can move incrementally without changing every overlay behavior at once.
56. As a Niribar maintainer, I want the first slices to preserve existing user behavior, so that the architecture refactor does not become a product rewrite.
57. As a Niribar maintainer, I want domain documentation to use **Foreground**, **Selectable Menu Session**, **Menu Search**, **Custom Menu Value**, and **SNI Menu**, so that future agents do not reintroduce old vocabulary.
58. As a Niribar maintainer, I want ADR-0008 to guide future architecture work, so that ADR-0007's old split is not treated as the current target.
59. As a Niribar maintainer, I want fewer sibling runtimes coordinated by the app root, so that app-level update code becomes easier to navigate.
60. As a Niribar maintainer, I want a deletion test for the new architecture to show depth, so that deleting **Foreground** would reintroduce replacement, stale-window, and interaction logic across many callers.

## Implementation Decisions

- Introduce **Foreground** as the deep module for interactive overlay state.
- **Foreground** has exactly one active state at a time: no overlay, **Menu**, or **Panel**.
- **Foreground** owns physical overlay window identity for interactive **Menus** and **Panels**.
- **Tooltip** remains outside **Foreground** because it is passive, non-interactive, and owned as a **Bar Surface** affordance.
- Every request that opens, toggles, or closes a **Panel** or **Menu** goes through a **Foreground** intent interface.
- Configured **Panel Actions**, tray menu requests, keyboard Escape, focus loss, and IPC-triggered panel toggles all route through **Foreground**.
- **Foreground** replaces the existing coordination pattern between panel runtime state, menu popup state, overlay lifecycle helpers, and the app root.
- **Foreground** is responsible for replacement semantics: opening a **Panel** or **Menu** replaces the current **Foreground**.
- **Foreground** is responsible for stale close semantics: a close event only affects the current **Foreground** when it matches the current physical overlay window identity.
- **Foreground** is responsible for producing physical window effects for interactive **Panel** and **Menu** windows.
- The **Bar Surface Registry** should narrow toward live **Bar Surface** records and passive **Tooltip** metadata.
- The **Bar Surface Registry** should no longer be the owner of interactive **Panel** or **Menu** metadata after the migration.
- Introduce **Selectable Menu Session** as the deep module for selectable **Menu** and **Panel Menu** interaction.
- **Selectable Menu Session** owns active menu level, pointer hover, keyboard focus, optional **Menu Search** text, optional **Custom Menu Value** affordance, and selected-entry completion.
- **Selectable Menu Session** owns nested flattening and breadcrumb context while **Menu Search** is active.
- **Selectable Menu Session** returns a generic selected entry rather than app-specific effects.
- The owning adapter interprets a generic selected entry as a **Panel Action**, **Installed Desktop Application** launch, **Dmenu Selection Request** response, or **SNI Menu** item activation.
- **Selectable Menu Session** does not own physical **Panel** or **Menu** window lifecycle.
- **Selectable Menu Session** does not know about DBusMenu, process launch, IPC responders, or app command execution.
- **Panel Menu Session** becomes a domain synonym for a **Selectable Menu Session** opened inside a **Panel**.
- Existing **Panel Menu** behavior migrates to **Selectable Menu Session** without changing user-facing behavior.
- Existing **Dmenu Selection Request** behavior migrates to **Selectable Menu Session** without changing user-facing behavior.
- **Dmenu Selection Mode** keeps **Custom Menu Value** behavior by default and remains configurable with `--no-custom`.
- Add tray **Module** configuration for enabling **Menu Search** on **SNI Menus**.
- **SNI Menus** do not show **Menu Search** by default.
- A **SNI Menu** with configured **Menu Search** renders a visible search input while open.
- A searchable **SNI Menu** uses the same **Menu Search** matching, ranking, nested-entry, and breadcrumb behavior as every other searchable menu.
- When a **SNI Menu** layout updates while open, its selectable session resets to top level with empty **Menu Search** text and initial focus.
- **Menu Search** remains fuzzy, ASCII case-insensitive, tokenized on whitespace, source-order preserving for blank queries, and source-order tie-breaking for equal scores.
- **Menu Search** continues to rank substring matches ahead of non-contiguous fuzzy matches when other priority rules are equal.
- **Menu Search** can match breadcrumb ancestor labels for nested entries as lowest-priority text.
- **Menu Search** for **Installed Desktop Applications** continues to match application names, descriptions, generic names, and keywords.
- The physical **Panel** presentation and physical **SNI Menu** popup presentation may differ, but both consume snapshots from the same selectable session module when rendering selectable entries.
- ADR-0008 is the architecture target for this PRD.
- ADR-0007 is superseded for future architecture work because the old split no longer handles shared **Selectable Menu Session** behavior cleanly.

## Testing Decisions

- The highest testing seam is **Foreground**: tests should send foreground intents and assert external effects, state snapshots, replacement behavior, and stale close behavior.
- The second testing seam is **Selectable Menu Session**: tests should send menu intents and assert snapshots and generic selected entries.
- Existing seams should be preferred where they already express behavior: panel lifecycle tests, menu popup tests, panel menu search tests, dmenu tests, and tooltip tests should be migrated rather than duplicated wholesale.
- New tests should avoid asserting internal storage maps, enum layout, or helper module structure unless those become explicit interfaces.
- Good **Foreground** tests assert that opening a **Panel** closes or replaces an active **Menu** through foreground behavior.
- Good **Foreground** tests assert that opening a **Menu** closes or replaces an active **Panel** through foreground behavior.
- Good **Foreground** tests assert that replacing a **Panel** makes stale close events from the old physical window harmless.
- Good **Foreground** tests assert that replacing a **Menu** makes stale close events from the old physical window harmless.
- Good **Foreground** tests assert that keyboard Escape routes through **Foreground** for **Menus** and **Panels**.
- Good **Foreground** tests assert that focus loss routes through **Foreground** where it closes an interactive overlay.
- Good **Foreground** tests assert that **Global Panel Toggle** routes through **Foreground**.
- Good **Foreground** tests assert that configured **Panel Actions** route through **Foreground**.
- Good **Selectable Menu Session** tests assert pointer hover and keyboard focus snapshots.
- Good **Selectable Menu Session** tests assert focus movement through visible selectable entries.
- Good **Selectable Menu Session** tests assert active-level navigation and back behavior for nested menus.
- Good **Selectable Menu Session** tests assert blank-query source order.
- Good **Selectable Menu Session** tests assert fuzzy search matching, token splitting, case-insensitivity, substring priority, and source-order tie-breaking.
- Good **Selectable Menu Session** tests assert nested search flattening with breadcrumb context.
- Good **Selectable Menu Session** tests assert **Custom Menu Value** placement after fuzzy matches and first when no entries match.
- Good **Selectable Menu Session** tests assert no **Custom Menu Value** appears when the source disables custom values.
- Good **Selectable Menu Session** tests assert activation returns a generic selected entry rather than a **Panel Action**, DBusMenu call, IPC response, or process launch.
- **Panel Menu** adapter tests should assert generic selected entries are interpreted as **Panel Actions**.
- Installed-applications adapter tests should assert generic selected entries are interpreted as app launch requests.
- **Dmenu Selection Request** adapter tests should assert generic selected entries are interpreted as selected IPC values.
- **SNI Menu** adapter tests should assert generic selected entries are interpreted as DBusMenu item activation.
- Searchable **SNI Menu** tests should assert that enabling tray menu search renders a visible search input.
- Searchable **SNI Menu** tests should assert that search is disabled by default.
- **SNI Menu** layout update tests should assert reset to top level, empty search, and initial focus.
- **Tooltip** tests should remain separate and should assert passive hover behavior through the **Bar Surface** affordance seam.
- **Bar Surface Registry** tests should be updated to assert live **Bar Surface** and passive **Tooltip** behavior rather than interactive **Panel** and **Menu** ownership after migration.
- Prior art exists in existing panel runtime tests for stale panel close and replacement behavior.
- Prior art exists in existing panel menu session tests for search, focus, activation, dmenu, installed applications, and nested menu behavior.
- Prior art exists in existing menu tests for SNI menu hover, keyboard focus, activation, layout updates, and popup close behavior.
- Prior art exists in existing surface registry tests for overlay replacement and close effects; those tests should migrate to **Foreground** where they concern **Panel** or **Menu** behavior.

## Out of Scope

- Changing **Tooltip** into **Foreground**.
- Changing user-facing **Tooltip** behavior.
- Changing **Bar**, **Bar Surface**, **Bar Surface Identity**, **Output**, **Output Target**, **Edge**, or **Layer** semantics.
- Changing **Panel** visual layout beyond what is needed to consume selectable menu snapshots.
- Changing **SNI Menu** visual layout when **Menu Search** is disabled.
- Enabling **Menu Search** for **SNI Menus** by default.
- Adding **Menu Search** to **Tooltips**.
- Adding multiple simultaneous **Foreground** overlays.
- Supporting multiple active **Selectable Menu Sessions**.
- Changing **Dmenu Selection Mode** CLI flags beyond preserving existing custom-value behavior.
- Changing **Dmenu Entry Stream** parsing.
- Changing rich dmenu JSON Lines input.
- Changing installed desktop application discovery rules.
- Changing centralized process launch policy.
- Introducing third-party fuzzy search as public domain behavior.
- Splitting the app into multiple crates.
- Building a plugin system.
- Rewriting unrelated **Module Runtime** or **Module Data Source** behavior.
- Implementing new **Panel Menu Source** kinds beyond what is required for this architecture.

## Further Notes

- `CONTEXT.md` has been updated with **SNI Menu**, **Menu Search**, **Custom Menu Value**, **Selectable Menu Session**, and **Foreground**.
- ADR-0008 records the architectural decision to model interactive overlays as **Foreground**.
- ADR-0007 is marked superseded for future architecture work.
- The main architectural goal is **Locality**: interactive overlay lifecycle and selectable menu behavior should be concentrated in deep modules instead of spread through app handlers, registry maps, panel runtime, menu popup helpers, and input routing.
- The main caller benefit is **Leverage**: **Panel Menus**, **Dmenu Selection Requests**, and searchable **SNI Menus** should use one selectable menu implementation.
- The deletion test for **Foreground** should be strong: deleting it would reintroduce open/toggle/close routing, replacement semantics, physical interactive overlay identity, and stale close handling across many callers.
- The deletion test for **Selectable Menu Session** should be strong: deleting it would reintroduce focus, hover, search, nested flattening, custom values, and generic selection behavior across **Panel Menu**, **Dmenu Selection Request**, and **SNI Menu** paths.
