Status: ready-for-agent

# Add the Module Detail Session Seam

## Parent

.scratch/module-detail-session/PRD.md

## What to build

Add the generic **Module Detail Session** seam for module-originated **Module Detail Content**. **Panel Runtime** should be able to create, store, replace, and discard an opaque detail session while preserving existing behavior for **Modules** that do not provide custom detail state.

This slice should establish the seam end-to-end without migrating clock behavior yet. A completed slice should make it possible for a **Module** to opt into a fresh detail session when its module-originated **Panel** opens, while simple **Modules** continue using the existing expanded/panel rendering behavior.

## Acceptance criteria

- [ ] A **Module** can optionally create a fresh **Module Detail Session** when module-originated **Module Detail Content** opens.
- [ ] **Panel Runtime** stores the active **Module Detail Session** opaquely and discards it when the **Panel** closes, toggles closed, is replaced, or switches to non-module detail content.
- [ ] **Modules** without custom detail behavior continue rendering their existing panel/detail content without requiring new boilerplate.
- [ ] Opening the same module-originated **Panel** again creates a fresh session rather than reusing previous detail state.
- [ ] Tests cover session creation, replacement, reset-on-open, and no-custom-detail fallback behavior through the new seam.

## Blocked by

None - can start immediately
