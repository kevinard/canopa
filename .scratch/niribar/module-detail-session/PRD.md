Status: ready-for-agent

# PRD: Module-Owned Detail Sessions

## Problem Statement

Niribar's **Module Detail Content** is starting to leak module-specific interaction state into generic **Panel Runtime** code. The clock calendar is a small feature, but adding it required generic panel messages, generic panel state, app-level routing, and extra arguments on the shared **Module** interface.

From a maintainer's perspective, this makes richer **Modules** expensive to extend. A fully featured network **Module** may need available network lists, selected network state, password input, connection progress, and connect/disconnect behavior. A richer audio **Module** may need input and output source lists, selected device state, and source switching. If those concerns follow the clock calendar pattern, the generic **Panel Runtime** will accumulate per-**Module** detail state and events that it should not understand.

## Solution

Deepen the **Module Detail Content** seam by introducing a module-owned **Module Detail Session**.

A **Module Detail Session** is the transient state and interaction lifecycle for **Module Detail Content** while its **Panel** is open. It is created fresh when module-originated **Module Detail Content** opens and discarded when that **Panel** closes or is replaced.

**Panel Runtime** stores the active **Module Detail Session** because its lifetime matches the open **Panel**, but it treats the session as opaque. The originating **Module** owns the detail session's meaning, detail rendering, detail messages, and module-specific effects. The app only routes opaque detail messages back to the active originating **Module** and executes app-owned effects when a **Module** requests shared infrastructure such as command execution, process launch policy, panel closing, menu opening, or focus behavior.

The first implementation should migrate the clock calendar to this seam as a tracer bullet. That should remove clock-specific state and clock-specific detail events from generic **Panel Runtime** and prove that future network/audio detail views can be added without expanding generic panel code per **Module**.

## User Stories

1. As a Niribar maintainer, I want **Module Detail Content** state to be owned by the originating **Module**, so that generic **Panel Runtime** does not accumulate per-**Module** concepts.
2. As a Niribar maintainer, I want **Panel Runtime** to store an opaque **Module Detail Session**, so that session lifetime follows the open **Panel** without making the panel interpret module-specific state.
3. As a Niribar maintainer, I want a **Module Detail Session** to be created fresh when module-originated **Module Detail Content** opens, so that detail views reset predictably.
4. As a Niribar user, I want the clock calendar to reset to the current local month each time I open the clock **Module Detail Content**, so that reopening the panel starts from a useful default.
5. As a Niribar user, I want closing and reopening a **Module** panel to discard old detail interaction state, so that stale selections do not carry across panel sessions.
6. As a Niribar maintainer, I want replacing one open **Panel** with another to discard the previous **Module Detail Session**, so that stale detail messages cannot mutate an inactive view.
7. As a Niribar maintainer, I want detail messages to be opaque at the app seam, so that the app does not need `Clock`, `Network`, or `Audio` detail event variants.
8. As a Niribar maintainer, I want the app to route opaque detail messages only to the active originating **Module**, so that stale messages from a replaced panel are ignored.
9. As a Niribar maintainer, I want the originating **Module** to interpret its own detail messages, so that detail interaction logic has strong **Locality**.
10. As a Niribar maintainer, I want the clock **Module** to own calendar month browsing state, so that generic **Panel Runtime** no longer has a clock-specific month offset.
11. As a Niribar maintainer, I want clock detail pointer controls to emit clock-owned detail messages, so that clock-specific events do not appear in generic panel messages.
12. As a Niribar user, I want clock month previous, next, and today controls to continue working, so that the refactor preserves existing behavior.
13. As a Niribar maintainer, I want keyboard behavior for clock month browsing to remain supported through the new detail seam, so that accessibility behavior survives the migration.
14. As a Niribar maintainer, I want **Panel Runtime** to continue owning open, close, toggle, focus, Escape, and panel window effects, so that generic panel lifecycle rules stay centralized.
15. As a Niribar maintainer, I want **Modules** to be able to return app-owned effects from detail interactions, so that shared app infrastructure is reused instead of reimplemented.
16. As a Niribar maintainer, I want command execution requested from **Module Detail Content** to go through the centralized app runner, so that command policy remains enforced consistently.
17. As a Niribar maintainer, I want module-owned effects to remain available for domain-specific work, so that future network and audio detail views can use their own runtime adapters for connect or source-switch behavior.
18. As a Niribar maintainer, I want app-owned effects and module-owned effects to be distinguishable, so that ownership of side effects is clear.
19. As a Niribar maintainer, I want a future network **Module** detail view to add available-network state without editing generic **Panel Runtime** for network concepts, so that the detail seam has real **Leverage**.
20. As a Niribar maintainer, I want a future audio **Module** detail view to add input/output source selection without editing generic **Panel Runtime** for audio concepts, so that richer **Modules** stay maintainable.
21. As a Niribar maintainer, I want the shared **Module** interface to make default simple detail behavior cheap, so that display-only **Modules** do not need boilerplate.
22. As a Niribar maintainer, I want **Modules** without custom **Module Detail Content** to keep rendering their existing panel view, so that the migration can be incremental.
23. As a Niribar maintainer, I want **Module Detail Session** tests to exercise behavior through the new seam, so that tests describe detail behavior rather than generic panel internals.
24. As a Niribar maintainer, I want old tests tied to clock-specific **Panel Runtime** internals removed or rewritten, so that tests do not preserve the shallow seam.
25. As a Niribar maintainer, I want the new seam documented in the domain language, so that future architecture reviews use **Module Detail Session** consistently.

## Implementation Decisions

- Introduce **Module Detail Session** as the domain concept for transient module-owned detail state while a module-originated **Panel** is open.
- **Module Detail Session** resets each time module-originated **Module Detail Content** opens.
- Store the active **Module Detail Session** in **Panel Runtime** because the session lifetime is exactly the open **Panel** lifetime.
- Keep the stored **Module Detail Session** opaque to **Panel Runtime**.
- **Panel Runtime** remains responsible for generic panel lifecycle: open, close, toggle, replacement, focus, Escape behavior, search focus where relevant, and panel window effects.
- The originating **Module** owns detail session initialization, detail rendering, detail message interpretation, and module-specific detail effects.
- The app routes opaque detail messages back to the active originating **Module**.
- The app drops an opaque detail message if its **Module Name** or runtime target does not match the active **Panel Origin**.
- Avoid adding a public app-level enum with clock/network/audio detail variants.
- Add optional detail hooks to the shared **Module** interface so a **Module** can open a fresh detail session, render detail content from that session, and handle opaque detail messages for that session.
- Keep default behavior for **Modules** that do not implement custom detail behavior, likely by continuing to render the existing expanded/panel view without custom session state.
- Add a detail effect result that can represent app-owned effects and module-owned effects.
- App-owned effects should cover shared app infrastructure such as centralized command execution, panel close/open behavior, menu opening, focus requests, and process launch policy.
- Module-owned effects should cover domain-specific work owned by the originating **Module** or its runtime adapter.
- A **Module** must not reimplement existing centralized app infrastructure merely because the effect originated in **Module Detail Content**.
- Migrate the clock calendar first as a tracer bullet.
- Clock month offset should move out of generic **Panel Runtime** and into the clock-owned **Module Detail Session**.
- Clock previous, next, and today detail interactions should move out of generic panel messages and become clock-owned detail messages.
- Existing clock detail behavior should remain: show the current local month by default, browse months while open, return directly to today, mark today when visible, start weeks on Monday, and leave outside-month cells blank.
- This PRD does not introduce a crate split or plugin system. The internal seam should be deepened first; crate extraction can be reconsidered only after the internal **Module** interface stabilizes.

## Testing Decisions

- Tests should verify behavior through the **Module Detail Session** seam instead of asserting generic panel implementation details.
- A good test should observe external behavior: session creation on open, session reset on reopen, stale message dropping, rendered state changes after detail messages, and emitted effects.
- Tests should not assert the internal storage shape used by **Panel Runtime** unless that shape becomes part of the interface.
- **Panel Runtime** should have focused tests for creating, replacing, and discarding opaque **Module Detail Sessions** as panels open, close, toggle, and switch content.
- **Panel Runtime** tests should verify it does not interpret module-specific detail messages or state.
- App-level routing tests should verify opaque detail messages are delivered only to the active originating **Module**.
- App-level routing tests should verify stale detail messages are ignored after the **Panel** closes or is replaced.
- Clock **Module** tests should cover clock detail session initialization, previous-month browsing, next-month browsing, today reset, and render-relevant state.
- Clock migration tests should cover the existing behavior currently protected by clock calendar and panel runtime tests.
- Tests around centralized command execution should be added when a **Module Detail Content** caller first returns an app-owned command effect.
- Prior art exists in current **Panel Runtime** tests for panel lifecycle and current clock tests for calendar behavior; those tests should be migrated toward the new seam where they currently assert clock-specific generic panel state.
- Future network/audio detail work should add tests against their module-owned detail sessions and runtime adapters, not generic **Panel Runtime** internals.

## Out of Scope

- Implementing the full network **Module** detail view.
- Implementing the full audio source/input/output detail view.
- Adding third-party or dynamic plugin loading.
- Splitting Niribar into multiple crates.
- Replacing the existing **Panel Menu** session model.
- Changing **Dmenu Selection Mode** behavior.
- Changing **Bar**, **Bar Surface**, or **Output Target** semantics.
- Changing clock calendar user-facing behavior beyond the ownership refactor.
- Adding persistent **Module Detail Content** state across panel closes.
- Letting generic **Panel Runtime** interpret clock, network, audio, or other module-specific detail events.

## Further Notes

- The domain glossary has been updated with **Module Detail Session**.
- The main architectural goal is **Locality**: detail behavior should change inside the owning **Module**, not across generic panel/app files.
- The main caller benefit is **Leverage**: future rich **Modules** can add detailed interaction without expanding the generic panel seam per **Module**.
- The deletion test for the current clock calendar is the signal for this refactor: deleting the calendar would remove clock-specific fields and events from generic panel code. After this PRD, deleting a **Module** detail implementation should mainly affect that **Module**.
