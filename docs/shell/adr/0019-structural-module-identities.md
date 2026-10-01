# Structural module identities and indexed placement

Configured Module names and Dynamic Module Child Keys occupy separate identity
namespaces. `ModuleId` owns immutable, shared identity data: either a configured
name or a parent name plus a typed workspace/tray child key. Delimiters in names
and tray keys have no identity meaning. Widget lookup uses the complete structural
identity as well as the window, so a configured `workspace:1` cannot alias the
workspace child numbered 1.

The identity is the source of parent ownership. Module Runtime no longer stores a
second origin map, and child modules derive their configuration name from their
identity. Configuration lookup borrows that name without building reconciliation
state.

Module Store maintains configured-name and ordered-child indexes alongside its ID
and kind indexes. Bar and Panel placement reads those lightweight indexes; changes
to store order rebuild them together. Dynamic Module Runtime Snapshots are reserved
for reconciliation, where actions and family configuration are needed. Ordinary
layout and name lookup never clone those payloads.

The regression fixture with 256 configured modules, 32 name lookups and four layout
expansions previously copied 9,216 reconciliation entries. The indexed boundary
copies none. This measures avoided work, not desktop latency. Collision fixtures
exercise real workspace and tray reconciliation, both configuration orders,
independent inherited actions, widget identity, and focus/hover retirement.
