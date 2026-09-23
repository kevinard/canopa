# ADR 0011: Explicit Canopa panel protocol adapter

Status: Accepted

## Context

Canopa Notify evolved its declarative panel protocol to JSON-lines v4 while Niribar retained framed JSON v1. Both transports have real providers. Replacing the existing protocol would break the shipped Wi-Fi provider; guessing a protocol from command names or timing would make failures ambiguous.

## Decision

Extend ADR 0010 with explicit `protocol "canopa-v4"` selection on a Configured Panel Provider. Omitted protocol continues to mean `niribar-v1`. Canopa providers send initial replacement content without a host-first handshake. Each bounded JSON line must carry the supported protocol version and valid opaque identifiers.

Keep canonical Canopa wire types in Notify's `canopa-panel-protocol` crate and vendor a verbatim, drift-checkable snapshot here for standalone builds. Keep transport decoding, generation-scoped interaction validation and bounded local-image preparation behind the Canopa adapter. The Foreground and External Panel Session retain their existing placement, lifecycle, stale-session filtering, and error behavior.

Canopa content is a host-only normalized content variant, not an extension accepted on the v1 wire. Niribar renders sections, controls, metadata, all actions, styled text and safe local images. Keyboard selection has a persistent contextual action affordance above the scrolling content. Invalid or replaced interaction IDs cannot invoke newly displayed actions.

Selection labels put the action/control before its section and item context. The full selected text remains in a bounded scrollable preview so a long provider title cannot consume the content viewport. Preview scroll and every action button's pointer-press state belong to the exact opaque interaction ID and reset when it changes. A press begun on old content cannot complete against its replacement; no notification-domain keys are interpreted by the widget. The Canopa widget also exposes native AccessKit/AT-SPI roles, names, state, and actions; keyboard guidance complements those semantics.

Do not interpret notification-specific action keys. Do not load remote images or execute provider-supplied commands. Canopa v4 link interactions are sent to the provider as opaque actions; the provider owns URL opening and its policy. Canopa v4 has no close message; close stdin and apply the existing bounded process-exit grace.

## Consequences

The existing Wi-Fi protocol stays compatible. Notify and Niribar must run the source drift check when upgrading Canopa. Graphical verification is separate from headless codec tests; image support does not itself justify new Notify capability advertisement. Native screen-reader integration now exists for Canopa panels. Broader desktop accessibility assessment still requires manual verification.
