# 0156. Share Files presentation with the portal picker

Status: superseded by 0157

## Context

The FileChooser portal already uses the Files browsing model. It had its own
breadcrumbs and file rows, so the two interfaces diverged. The same
picker serves Account and Print, including the Print Recovery path.

## Decision

Files exposes its breadcrumb and file-row widgets, browser chrome, input
styles and built-in glyphs through the `browser-ui` Cargo feature. The portal
enables `browser-ui` with Files' default features disabled. The enclosing
portal dialog sets the appearance. The picker prepares glyphs from the shared
definitions and uses shared-mime-info filename hints to categorize rows.

The chooser supplies request-scoped navigation and selection actions to the
shared widgets. The portal owns picker modes, caller options, review and
acceptance. Its other native dialogs share a presentation module for the
surface, controls and action footer. Each dialog owns the wording and actions of its request.

## Consequences

Changes to the breadcrumb, responsive file labels and row selection cues
reach both hosts through the same implementation. The portal can
load the shared presentation without starting the Files resident application,
linking its GIO runtime or taking ownership of file operations. The frontend
continues to grant the files selected by the portal.
