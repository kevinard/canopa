# Tray Keyboard Navigation Design

**Date:** 2026-04-17
**Author:** Kevin

## Summary

Add full keyboard navigation for tray icons and their popup context menus. Enter and Space are split into distinct interactions: Enter activates (left-click equivalent), Space opens the context menu (right-click equivalent). The popup menu is navigable with arrow keys, activated with Enter, and dismissed with Escape.

---

## Section 1: InteractionEvent Split

### New enum variant

Add `Activated` to `InteractionEvent`:

```rust
pub enum InteractionEvent {
    Clicked,     // left-click / Enter key
    Activated,   // Space key -- secondary activation (open menu, etc.)
    ScrollUp,
    ScrollDown,
}
```

### Input mapping changes

`keyboard_handler` in `crates/niribar/src/surface/input.rs` currently maps `Enter | Space` to a single `KeyboardActivate` message. This changes to:

| Key | Message |
|-----|---------|
| Enter | `KeyboardActivate(window)` |
| Space | `KeyboardSecondaryActivate(window)` |

### Message handling

**`KeyboardActivate` (Enter):** Unchanged behavior -- calls `module.on_interaction(InteractionEvent::Clicked)`. The existing two-stage check (scrollable first, then clicked) is preserved.

**`KeyboardSecondaryActivate` (Space):** New handler -- calls `module.on_interaction(InteractionEvent::Activated)`. If the module returns `Some(msg)`, execute it. No interaction-mode toggle (Space is a one-shot action, not a mode).

### Module implementations

All existing modules are unaffected -- `Activated` falls through to `None` via `_ => None`.

`TrayIconModule::on_interaction` maps:
- `Clicked` -> `TrayActivate { service }` (left-click: activate the SNI item)
- `Activated` -> `TrayMenuRequest { service, menu_path }` (right-click: open context menu)

---

## Section 2: Popup Keyboard Navigation

### State

Add to `App`:

```rust
pub menu_focused_item: HashMap<iced::window::Id, String>,
```

Tracks which menu item has keyboard focus per popup window. Separate from `menu_popup_hovered` (mouse hover). Starts empty when popup opens -- no item highlighted until first arrow press.

### Input routing

`keyboard_handler` does not need access to `App` state. Arrow keys, Enter, and Escape produce the same messages as in bar mode (`FocusMoveNext`, `FocusMovePrev`, `KeyboardActivate`, `EscapePressed`). Each message handler in `update()` checks `self.menu_popups.contains_key(&window)` and dispatches to menu-specific logic when the window is a popup.

### Menu navigation logic

Navigation operates on a filtered list: visible, enabled, non-separator items from the popup's `MenuTree`.

| Action | Behavior |
|--------|----------|
| Arrow Down | Move to next item in filtered list. Wrap from last to first. If no item focused, select first. |
| Arrow Up | Move to previous item. Wrap from first to last. If no item focused, select last. |
| Enter | If an item is focused: fire `MenuItemSelected` for that item, close popup, clear bar focus. If no item focused: do nothing. |
| Escape | Close popup (`MenuClosed`), clear bar focus. |

### Rendering

`view_menu_popup` already accepts `hovered_item: Option<&str>` for highlight styling. The keyboard-focused item uses the same visual treatment. The view merges both sources: mouse hover takes priority over keyboard focus. When the mouse moves, it overrides keyboard focus highlight.

Signature change:

```rust
pub fn view_menu_popup<'a>(
    tree: &'a MenuTree,
    service: &'a str,
    menu_path: &'a zbus::zvariant::OwnedObjectPath,
    window: iced::window::Id,
    hovered_item: Option<&'a str>,
    focused_item: Option<&'a str>,   // new parameter
) -> iced::Element<'a, crate::app::Message>
```

Highlight logic: `hovered_item.or(focused_item)` determines which item gets the `SURFACE0` background.

---

## Section 3: Focus Lifecycle

### Opening the popup

1. Space key on focused tray icon fires `KeyboardSecondaryActivate`
2. `TrayIconModule::on_interaction(Activated)` returns `TrayMenuRequest`
3. Menu fetch and `OpenMenu` flow is unchanged
4. Popup window opens with `KeyboardInteractivity::OnDemand`
5. `menu_focused_item` starts empty -- no highlight until first arrow press

### Closing the popup

| Trigger | Behavior |
|---------|----------|
| Escape in popup | `EscapePressed(window)` -> if popup: fire `MenuClosed`, clear bar focus on all surfaces |
| Enter on focused item | `KeyboardActivate(window)` -> if popup and item focused: fire `MenuItemSelected`, close popup, clear bar focus |
| Enter with no item focused | Do nothing |
| Mouse click on item | Existing flow unchanged; bar focus cleared |
| Outside click | Existing `MenuClosed` flow unchanged |

### Focus clearing on close

After popup teardown, clear `focused_module` and `module_activated` on all surfaces. The user must re-focus the bar to continue keyboard navigation.

---

## Section 4: Files Modified

| File | Change |
|------|--------|
| `crates/niribar/src/modules.rs` | Add `Activated` variant to `InteractionEvent` |
| `crates/niribar/src/surface/input.rs` | Split `Enter \| Space` into separate messages |
| `crates/niribar/src/app.rs` | Add `KeyboardSecondaryActivate` message and handler; update `KeyboardActivate`, `FocusMoveNext`, `FocusMovePrev`, `EscapePressed` to check for popup context; add `menu_focused_item` field |
| `crates/niribar/src/modules/tray.rs` | Handle `Activated` in `TrayIconModule::on_interaction` |
| `crates/niribar/src/surface/menu_popup.rs` | Add `focused_item` parameter to `view_menu_popup` |
