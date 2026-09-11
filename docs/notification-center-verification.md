# Notification Center verification

The compatible host is the Canopa-v3 Niribar port described in [provider setup](niribar-panel-provider.md). Automated checks validate the wire stream, content identifiers, typed interactions, snapshot generations, keyboard-action selection, bounded image preparation, and the existing Wi-Fi transport. They do not validate compositor rendering or accessibility integration.

Before enabling additional advertised body capabilities, run this checklist in a disposable or explicitly selected Wayland session. Do not replace a user's running bar/notification service as part of an automated headless test.

- Start Notify and the rebuilt Niribar with the provider configured as `canopa-v3`; open Notifications from a module action.
- Send several applications' notifications. Confirm section titles, summaries, metadata, permission controls, and all named actions are visible/reachable. Verify active state changes replace content without opening a second panel.
- Send a transient notification with four named actions and default activation while three critical banners occupy the stack. Confirm its runtime-only center item and every action are reachable by mouse and keyboard. Invoke resident actions, replace it, close it, and restart the daemon: no transient archive should remain. Repeat with DND enabled. Center opening must not start an undisplayed banner's timeout or change keyboard focus outside the deliberately opened panel.
- Send interleaved bold/italic/underlined text, a valid local PNG, and following text. Confirm ordering and styling. A valid image must not duplicate its alternative text; a missing/unsafe image must show its fallback.
- Send a long body and many actions. Confirm scrolling, wrapping and all actions remain usable. Verify arrows/Tab/Shift-Tab select actions, the persistent selected-action affordance identifies its notification/control, Enter invokes exactly that action, and Escape closes the panel.
- Replace an item while an action is selected. Confirm selection clears and an old click cannot invoke the new snapshot. Close/reopen the panel and confirm old-session clicks remain stale.
- Invoke resident and nonresident actions while watching the private test D-Bus signals. Confirm opaque action keys survive, resident items remain active, and token/action/closure order is preserved where token acquisition is available.
- Change an app/category permission, close and reopen the panel, and confirm the selected option persists. Confirm Clear dismisses active items and Delete History removes only inactive history.
- Stop the test daemon/provider and verify visible failure content, bounded cleanup, and successful reopening after restart. Verify normal Wi-Fi provider operation remains unchanged.

Status as of this implementation tranche: graphical checklist not yet executed; `body-images`, `body-hyperlinks`, and `persistence` remain unadvertised. ADR 0041 adds transient runtime-only items and all actions to the provider stream, not graphical proof. Screen-reader integration, actual transient overflow/action reachability, banner geometry, projection overflow/pagination, and safe hyperlinks remain tracked work.
