Status: ready-for-agent

# Send Typed Button Events And Apply Full Content Replacement

## Parent

[PRD: External Panel Provider Platform](../PRD.md)

## What to build

Add the first interactive **External Panel Content** primitive: a button with an opaque **External Panel Element ID**. When the user activates the button, Niribar sends a typed provider event. The provider can respond with a complete replacement content model, and Niribar updates the panel from that model.

This slice proves provider-owned state transitions while preserving Niribar-owned rendering and event routing.

## User stories covered

18, 21, 22, 26, 40

## Acceptance criteria

- [ ] Provider content can include a button primitive with an opaque **External Panel Element ID**.
- [ ] Activating the rendered button sends a typed activation event to the provider with the element ID.
- [ ] Niribar does not interpret the element ID as domain meaning.
- [ ] The provider can respond to the event with a complete replacement content model.
- [ ] Niribar replaces the displayed **External Panel Content** with the new full model.
- [ ] Provider content cannot ask Niribar to run arbitrary **Configured Commands**.
- [ ] Tests verify event serialization, provider event routing, and full-model replacement behavior.

## Blocked by

- [02 - Render Static External Panel Content After Handshake](02-render-static-external-panel-content-after-handshake.md)
