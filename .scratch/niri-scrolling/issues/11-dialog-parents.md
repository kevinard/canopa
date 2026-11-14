# Dialog parents in IPC

Status: implemented. Fork PR kevinard/niri#15, pinned (now `b4fc5ea7`); the
Shell's `niri-ipc` now comes from the same fork commit.

Branch `canopa/window-parent-ipc` (`5403b0c6` on `canopa/base`, PR #15):
"ipc: Add the parent window id to Window". `Window.parent_id: Option<u64>`
(`#[serde(default)]`, so older clients and servers interoperate) is the Niri ID
of the `xdg_toplevel.set_parent` window, or `null` when there is none or it is
not mapped yet. A parent change, including the parent mapping or closing later,
sends `WindowOpenedOrChanged`; `niri msg windows` prints it. Two fixture tests in
`src/tests/ipc_windows.rs`; event emission is not covered because the fixtures
have no IPC server.

Shell: a window whose parent belongs to the same application is a dialog of
it. The taskbar does not count it or cycle to it, a focused dialog still marks
its application focused, and a click while a dialog is in front cycles on from
its parent. Dialogs of other applications keep their own entry. Floating
dialogs were already left out of the minimap. Test:
`dialogs_belong_to_their_parent_window`.

The Shell's `niri-ipc` is the fork's (`[patch.crates-io]` at the compositor
pin); `scripts/test-fork-sources.py` checks the two match. That also brought
`Output::max_bpc` from the fork's base, so one Settings test literal changed,
and serde 1.0.229 / serde_json 1.0.154, which the fork's workspace requires.

## Comments
