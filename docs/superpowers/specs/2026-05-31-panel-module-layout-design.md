# Panel Module Layout Design

## Goal

Unify **Bar Panel** and **Module Panel** module behavior so both are treated as module-backed **Panels**: a **Panel** containing one or more **Modules**.

## Architecture

`PanelContent::BarPanel` and `PanelContent::ModulePanel(id)` remain as compatibility inputs, but module behavior resolves through one **Panel Module Layout** concept.

- `BarPanel` resolves to the configured `bar.panel.modules` layout for the originating **Bar Surface**.
- `ModulePanel(id)` resolves to one centered module entry.
- `Menu` and `DmenuRequest` do not resolve to a module layout.

Rendering, focus metadata, active-target checks, and **Module Detail Session** routing consume the resolved layout instead of branching separately on `BarPanel` and `ModulePanel`.

## Module Detail Sessions

`PanelRuntime` stores **Module Detail Sessions** by `ModuleId`, not as one global optional session. A module-backed **Panel** may therefore hold independent sessions for every contained **Module** that supports detail state.

When opening a module-backed **Panel**, `App` resolves its layout, asks each contained **Module** to create a session, and passes the resulting session map to `PanelRuntime`. Detail messages mutate only the session for their target `ModuleId` when that target is present in the active module-backed **Panel**.

## Data Flow

1. A **Panel Action** creates a `PanelOpenRequest`.
2. `App` resolves the request to a **Panel Module Layout** when the content is module-backed.
3. `App` creates per-module **Module Detail Sessions** for the resolved layout.
4. `PanelRuntime` stores the active **Panel** and the per-module session map.
5. Rendering resolves the same layout and passes each **Module** its own session.
6. Detail messages update only the matching active module session.

## Testing

The implementation is test-driven:

- A **Bar Panel** with two detail-capable **Modules** stores and updates independent sessions.
- Panel rendering passes each **Module** its own detail session.
- Existing single-module **Module Panel** behavior remains the one-module case.
- Existing **Panel Menu** and **Dmenu Selection Request** behavior remains unchanged.
