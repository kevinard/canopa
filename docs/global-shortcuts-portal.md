# GlobalShortcuts portal

Canopa has an experimental Rust backend for GlobalShortcuts v2, with Iced consent
and configuration. It is exported by the normal backend and is selectable with
`--extra-portals` in the [optional companion package](desktop-sharing.md). The
default descriptor does not list it. It requires the compatible Niri
shortcut-lease companion and standard portal frontend recorded in the
[evidence ledger](../.scratch/xdg-portals/global-shortcuts/session-evidence.json).

Applications create a session, provide action IDs and descriptions, and attempt
Bind once. Every action initially stays unchecked. Preferred and previously
saved triggers are editable suggestions. Explicit approval can select all, some
or none of the actions. Configure reopens the current set; a conflict keeps the
same dialog open for correction. Cancel before Apply preserves existing bindings.
The editor currently accepts XDG trigger names such as `CTRL+ALT+a` or `LOGO+F9`.
Record keys also captures a shortcut in the current keyboard layout. Release its
keys to finish, or press Escape to cancel recording and keep the previous draft.
Recording preserves the action's checkbox and still requires separate approval.

The editor now requests standard Wayland shortcut inhibition for its own window.
While it has keyboard focus, the compatible Niri companion suspends shortcut
activation and retains the existing registrations. Another consent editor can
still open; cancellation preserves the previous reservations. The rebuilt normal
application passes 22 native groups across both appearances, with additional native
window-lifetime checks and shared desktop regressions recorded under
[ADR0068](adr/0068-editor-keyboard-shortcut-inhibition.md).

The compositor owns matching, conflicts and press/release recipients. Each
session owns one authenticated private lease; the Iced helper receives no
compositor descriptor. Only the original frontend caller may operate the
session. Caller/compositor loss, session close and ambiguous mutation outcomes
revoke its authority. Public activation signals preserve the original u64
millisecond source time. No activation token is fabricated.

Successful explicit choices are saved in
`$XDG_STATE_HOME/canopa/portal-shortcuts.db`, or
`$HOME/.local/state/canopa/portal-shortcuts.db`. Saved choices are separate from
live authority. Before Bind, List describes the app's previous choices; after an
attempt it describes this session's actual live set. A canceled attempt consumes
Bind and leaves an empty set. A new session requires new approval.

The recording implementation passes87 canonical native groups across US
dark/100% and light/125%, French and Caps-as-Control, including actual activation,
held-key/repeat handling, Escape, focus loss and conflict retry. Native lifetime
checks and all six shared desktop suites pass. The completed workspace gate
retains the same ten Files failures; other Rust targets and all post stages pass.
Minimum-window/long-label, broader physical/layout/policy, sandbox and distribution
acceptance remain open; see [ADR0069](adr/0069-source-owned-shortcut-recording.md).
The normal provider selection remains unchanged while this work proceeds.

Focused development checks reuse the root default-feature incremental cache:

```sh
CARGO_BUILD_JOBS=1 cargo test --locked --offline -p canopa-portal --lib --test integration
CARGO_BUILD_JOBS=1 cargo clippy --locked --offline -p canopa-portal --all-targets -- -D warnings
cargo fmt --package canopa-portal -- --check
```

Shared SDK changes require `bash scripts/check.sh --workspace` on the original
whole-workspace feature/profile selection and root cache. Keep that selection
when continuing a broad check; excluding applications can build another graph.

Socket and D-Bus tests need an environment that permits their private IPC.
`scripts/test-global-shortcuts-portal.py` takes exact application, compositor and
frontend checksums and uses a private nested desktop, private state and temporary
portal routing. It does not install or select a provider for the live session.
Its current acceptance covers dark/100% and light/125%, actual Iced review,
conflict retry, source-time signals, cancellation/history, lease release and
helper socket isolation. See [ADR0067](adr/0067-shortcut-sessions-and-configuration.md)
for ownership and the exact receipts for validation scope.
