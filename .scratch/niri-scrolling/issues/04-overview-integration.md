# Overview integration

Status: implemented. Fork PR kevinard/niri#17, pinned (now `b4fc5ea7`).

Done: the Shell wallpaper is placed within the overview backdrop
(`place-within-backdrop` layer rule in `packaging/niri/defaults.kdl`, issue 05),
and the top-left hot corner opens the overview.

Typing to search cannot be done in the Shell alone. While the overview is open,
Niri gives keyboard focus to the overview itself (`KeyboardFocus::Overview`),
handles only Escape, Return and the arrow keys (`hardcoded_overview_bind` in
`src/input/mod.rs`) and drops every other key, so no client sees the typing.

Proposed fork change: when the overview has focus and an unbound key produces
text, Niri closes the overview and runs a configured action with that text, or
emits an IPC event the Shell answers by opening the launcher with the text as
its query. The second keeps the policy in the Shell; it needs the launcher to
accept an initial query from the existing `--toggle-menu` IPC.

## Fork

Branch `canopa/overview-typing` (`eeb43db1` on `canopa/global-shortcuts`, PR #17): "ipc: Report text typed into the
overview". With `overview { emit-typed-text; }` (off by default, since it sends
typed text to every event-stream client), a key press in the overview with no
bind or global shortcut that produces text, without Ctrl, Alt or Super, emits
`{"OverviewTyped":{"text":"a"}}` to event-stream clients, one per press, at most
16 bytes, no control characters; the overview stays open. Three filter unit
tests and three integration tests.

## Shell

On `OverviewTyped` the Shell opens the launcher menu (the menu whose source is
`launcher`) with the text, over the overview: Niri gives keyboard focus to
top-layer surfaces before the overview, so later keys reach the launcher
directly, and keys Niri reports before it maps extend the query. A query that
arrives while the launcher loads its sources is kept by the panel runtime and
applied when its session opens. Closing the launcher (a result or Escape)
closes the overview. `packaging/niri/defaults.kdl` turns on
`overview { emit-typed-text; }`; the packaged Niri 26.04 rejects that line, so
the defaults now need the pinned compositor.

## Validation

- `cargo test -p canopa-shell`: overview typing opens the launcher with the
  text, extends it without reopening, keeps a query that arrives before the
  session opens, and closing the launcher ends the search.
- Private Weston/Niri session with the pinned build (`bb457fce`): `wtype -d 150
  fix` in the overview gave the query "fix" with the overview still open, and two
  Escapes (clear, close) closed the launcher and the overview. Before the
  keep-the-overview-open change, keys typed while it closed reached the
  application window instead. Evidence in
  `.scratch/artifacts/niri-scrolling/pin-features`.

## Comments
