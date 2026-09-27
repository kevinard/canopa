# Tray Keyboard Navigation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add full keyboard navigation for tray icons and popup menus -- Enter activates, Space opens the menu, arrow keys navigate menu items, Enter selects, Escape closes.

**Architecture:** Split `InteractionEvent` to distinguish Enter (Clicked) from Space (Activated). Popup keyboard input reuses existing message types (`FocusMoveNext`, `FocusMovePrev`, `KeyboardActivate`, `EscapePressed`) with popup-aware routing in `update()`. Menu focus state tracked in `App::menu_focused_item`.

**Tech Stack:** Rust, iced 0.14, iced_layershell 0.17.1

---

## File Structure

| File | Action | Purpose |
|------|--------|---------|
| `crates/niribar/src/modules.rs` | Modify | Add `Activated` variant to `InteractionEvent` |
| `crates/niribar/src/surface/input.rs` | Modify | Split Enter/Space into `KeyboardActivate`/`KeyboardSecondaryActivate` |
| `crates/niribar/src/app.rs` | Modify | Add `KeyboardSecondaryActivate` message + handler; popup-aware routing in existing handlers; `menu_focused_item` field; focus clearing on popup close |
| `crates/niribar/src/modules/tray.rs` | Modify | Handle `Activated` in `TrayIconModule::on_interaction` |
| `crates/niribar/src/surface/menu_popup.rs` | Modify | Add `focused_item` parameter to `view_menu_popup` |

---

## Task 1: Add `Activated` variant to `InteractionEvent`

**Files:**
- Modify: `crates/niribar/src/modules.rs:60-67`

- [ ] **Step 1: Write failing test**

Add to the `#[cfg(test)]` block in `crates/niribar/src/modules.rs`:

```rust
#[test]
fn interaction_event_activated_variant_exists() {
    let _ = InteractionEvent::Activated;
}
```

- [ ] **Step 2: Run test to verify it fails**

```bash
cargo test interaction_event_activated 2>&1 | tail -5
```
Expected: FAIL -- no variant `Activated`

- [ ] **Step 3: Add the variant**

In `crates/niribar/src/modules.rs`, change:

```rust
pub enum InteractionEvent {
    Clicked,
    ScrollUp,
    ScrollDown,
}
```

to:

```rust
pub enum InteractionEvent {
    Clicked,
    Activated,
    ScrollUp,
    ScrollDown,
}
```

- [ ] **Step 4: Run tests**

```bash
cargo test 2>&1 | tail -5
```
Expected: all tests pass

- [ ] **Step 5: Commit**

```bash
git add crates/niribar/src/modules.rs
git commit -m "feat(modules): add Activated variant to InteractionEvent"
```

---

## Task 2: Split Enter/Space in keyboard handler

**Files:**
- Modify: `crates/niribar/src/surface/input.rs:116-118`
- Modify: `crates/niribar/src/app.rs` (Message enum, line ~140)

- [ ] **Step 1: Add `KeyboardSecondaryActivate` to the Message enum**

In `crates/niribar/src/app.rs`, after the existing `KeyboardActivate` message (line ~140):

```rust
    /// Enter or Space pressed while bar is focused (Story 6.3). Per-surface.
    KeyboardActivate(iced::window::Id),
```

Add:

```rust
    /// Space pressed while bar is focused -- secondary activation. Per-surface.
    KeyboardSecondaryActivate(iced::window::Id),
```

- [ ] **Step 2: Split Enter and Space in `keyboard_handler`**

In `crates/niribar/src/surface/input.rs`, change lines 116-118 from:

```rust
                keyboard::Key::Named(Named::Enter | Named::Space) => {
                    Some(Message::KeyboardActivate(window))
                }
```

to:

```rust
                keyboard::Key::Named(Named::Enter) => {
                    Some(Message::KeyboardActivate(window))
                }
                keyboard::Key::Named(Named::Space) => {
                    Some(Message::KeyboardSecondaryActivate(window))
                }
```

- [ ] **Step 3: Add stub handler for `KeyboardSecondaryActivate` in `update()`**

In `crates/niribar/src/app.rs`, add a handler after the `KeyboardActivate` handler (after line ~986):

```rust
            Message::KeyboardSecondaryActivate(window) => {
                let s = self.surface_mut(window);
                if s.focused_module.is_none() {
                    return Task::none();
                }
                if let Some(ref id) = s.focused_module.clone() {
                    for (_, module) in &self.modules {
                        if module.id() == *id {
                            if let Some(msg) = module.on_interaction(InteractionEvent::Activated) {
                                return Task::done(msg);
                            }
                            break;
                        }
                    }
                }
                Task::none()
            }
```

- [ ] **Step 4: Run tests**

```bash
cargo test 2>&1 | tail -5
```
Expected: all tests pass (existing `KeyboardActivate` tests still pass because Enter still maps to `KeyboardActivate`)

- [ ] **Step 5: Commit**

```bash
git add crates/niribar/src/surface/input.rs crates/niribar/src/app.rs
git commit -m "feat(input): split Enter/Space into KeyboardActivate/KeyboardSecondaryActivate"
```

---

## Task 3: Handle `Activated` in TrayIconModule

**Files:**
- Modify: `crates/niribar/src/modules/tray.rs:287-294`

- [ ] **Step 1: Write failing test**

Add to the `#[cfg(test)]` block in `crates/niribar/src/modules/tray.rs`:

```rust
#[test]
fn tray_icon_module_space_opens_menu() {
    let item = TrayItem {
        service: "org.test.App".into(),
        object_path: zbus::zvariant::OwnedObjectPath::try_from("/StatusNotifierItem").unwrap(),
        icon: TrayIcon::Named("app".into()),
        tooltip: None,
        status: ItemStatus::Active,
        menu_path: Some(
            zbus::zvariant::OwnedObjectPath::try_from("/com/canonical/dbusmenu").unwrap(),
        ),
    };
    let m = TrayIconModule::new(item, "tray".into());
    let msg = m.on_interaction(InteractionEvent::Activated).unwrap();
    assert!(matches!(
        msg,
        crate::app::Message::TrayMenuRequest { service, .. } if service == "org.test.App"
    ));
}
```

- [ ] **Step 2: Run test to verify it fails**

```bash
cargo test tray_icon_module_space_opens_menu 2>&1 | tail -5
```
Expected: FAIL -- `on_interaction(Activated)` returns `None`

- [ ] **Step 3: Implement the handler**

In `crates/niribar/src/modules/tray.rs`, change `on_interaction` (lines 287-294) from:

```rust
    fn on_interaction(&self, event: InteractionEvent) -> Option<crate::app::Message> {
        match event {
            InteractionEvent::Clicked => Some(crate::app::Message::TrayActivate {
                service: self.item.service.clone(),
            }),
            _ => None,
        }
    }
```

to:

```rust
    fn on_interaction(&self, event: InteractionEvent) -> Option<crate::app::Message> {
        match event {
            InteractionEvent::Clicked => Some(crate::app::Message::TrayActivate {
                service: self.item.service.clone(),
            }),
            InteractionEvent::Activated => Some(crate::app::Message::TrayMenuRequest {
                service: self.item.service.clone(),
                menu_path: self.item.menu_path.clone(),
            }),
            _ => None,
        }
    }
```

- [ ] **Step 4: Run tests**

```bash
cargo test 2>&1 | tail -5
```
Expected: all tests pass

- [ ] **Step 5: Commit**

```bash
git add crates/niribar/src/modules/tray.rs
git commit -m "feat(tray): handle Activated event to open menu via keyboard"
```

---

## Task 4: Add `menu_focused_item` state and popup-aware routing

**Files:**
- Modify: `crates/niribar/src/app.rs`

This task adds the `menu_focused_item` field to `App` and updates four existing message handlers to check for popup context.

- [ ] **Step 1: Add `menu_focused_item` field to `App`**

In `crates/niribar/src/app.rs`, after the `menu_popup_hovered` field (line ~82):

```rust
    /// Currently hovered menu item per popup window: window_id -> item_id string.
    pub menu_popup_hovered: HashMap<iced::window::Id, String>,
```

Add:

```rust
    /// Keyboard-focused menu item per popup window: window_id -> item_id string.
    pub menu_focused_item: HashMap<iced::window::Id, String>,
```

Initialize it in `boot()` alongside `menu_popup_hovered`:

```rust
            menu_focused_item: HashMap::new(),
```

- [ ] **Step 2: Write failing test for popup Escape**

Add to the `#[cfg(test)]` block in `crates/niribar/src/app.rs`:

```rust
#[test]
fn escape_in_popup_fires_menu_closed() {
    let (mut app, wid) = make_interact_app();
    // Simulate an open popup by inserting a fake entry.
    let menu_path =
        zbus::zvariant::OwnedObjectPath::try_from("/com/canonical/dbusmenu").unwrap();
    let tree = crate::surface::menu_popup::MenuTree {
        items: vec![crate::surface::menu_popup::MenuItem {
            id: "1".into(),
            label: "Open".into(),
            enabled: true,
            visible: true,
            icon_name: None,
            children: vec![],
            is_separator: false,
        }],
        anchor: (0, 0),
        screen_size: (0, 0),
    };
    let popup_wid = iced::window::Id::unique();
    app.menu_popups
        .insert(popup_wid, ("svc".into(), menu_path, tree, String::new()));
    // Escape on the popup window should close it.
    let _ = app.update(Message::EscapePressed(popup_wid));
    assert!(app.menu_popups.is_empty());
}
```

- [ ] **Step 3: Run test to verify it fails**

```bash
cargo test escape_in_popup 2>&1 | tail -5
```
Expected: FAIL -- popup is not removed because `EscapePressed` currently checks surface state, not popups

- [ ] **Step 4: Update `EscapePressed` handler for popup context**

In `crates/niribar/src/app.rs`, at the top of the `EscapePressed` handler (line ~862), add a popup check before the existing logic:

```rust
            Message::EscapePressed(window) => {
                // Popup context: Escape closes the menu and clears bar focus.
                if self.menu_popups.contains_key(&window) {
                    return Task::done(Message::MenuClosed);
                }
                let s = self.surface_mut(window);
                // ... rest of existing handler unchanged ...
```

- [ ] **Step 5: Run test to verify it passes**

```bash
cargo test escape_in_popup 2>&1 | tail -5
```
Expected: PASS

- [ ] **Step 6: Update `FocusMoveNext` handler for popup context**

At the top of the `FocusMoveNext` handler (line ~880), add popup routing:

```rust
            Message::FocusMoveNext(window) => {
                // Popup context: navigate to next menu item.
                if let Some((_, _, tree, _)) = self.menu_popups.get(&window) {
                    let navigable: Vec<&str> = tree
                        .items
                        .iter()
                        .filter(|i| i.visible && i.enabled && !i.is_separator)
                        .map(|i| i.id.as_str())
                        .collect();
                    if navigable.is_empty() {
                        return Task::none();
                    }
                    let current = self.menu_focused_item.get(&window).map(|s| s.as_str());
                    let next = match current {
                        None => navigable[0],
                        Some(c) => {
                            let idx = navigable.iter().position(|&id| id == c).unwrap_or(0);
                            navigable[(idx + 1) % navigable.len()]
                        }
                    };
                    self.menu_focused_item.insert(window, next.to_string());
                    return Task::none();
                }
                let s = self.surface_mut(window);
                // ... rest of existing handler unchanged ...
```

- [ ] **Step 7: Update `FocusMovePrev` handler for popup context**

At the top of the `FocusMovePrev` handler (line ~908), add popup routing:

```rust
            Message::FocusMovePrev(window) => {
                // Popup context: navigate to previous menu item.
                if let Some((_, _, tree, _)) = self.menu_popups.get(&window) {
                    let navigable: Vec<&str> = tree
                        .items
                        .iter()
                        .filter(|i| i.visible && i.enabled && !i.is_separator)
                        .map(|i| i.id.as_str())
                        .collect();
                    if navigable.is_empty() {
                        return Task::none();
                    }
                    let current = self.menu_focused_item.get(&window).map(|s| s.as_str());
                    let prev = match current {
                        None => navigable[navigable.len() - 1],
                        Some(c) => {
                            let idx = navigable.iter().position(|&id| id == c).unwrap_or(0);
                            navigable[(idx + navigable.len() - 1) % navigable.len()]
                        }
                    };
                    self.menu_focused_item.insert(window, prev.to_string());
                    return Task::none();
                }
                let s = self.surface_mut(window);
                // ... rest of existing handler unchanged ...
```

- [ ] **Step 8: Update `KeyboardActivate` handler for popup context**

At the top of the `KeyboardActivate` handler (line ~961), add popup routing:

```rust
            Message::KeyboardActivate(window) => {
                // Popup context: activate the focused menu item.
                if self.menu_popups.contains_key(&window) {
                    if let Some(item_id) = self.menu_focused_item.get(&window) {
                        let menu_item_id: i32 = item_id.parse().unwrap_or(0);
                        // Find the popup's service and menu_path.
                        if let Some((service, menu_path, _, _)) = self.menu_popups.get(&window) {
                            let msg = Message::MenuItemSelected {
                                service: service.clone(),
                                menu_item_id,
                                menu_path: menu_path.clone(),
                            };
                            return Task::done(msg);
                        }
                    }
                    // No item focused -- do nothing.
                    return Task::none();
                }
                let s = self.surface_mut(window);
                // ... rest of existing handler unchanged ...
```

- [ ] **Step 9: Write test for arrow-down navigation in popup**

Add to the `#[cfg(test)]` block in `crates/niribar/src/app.rs`:

```rust
#[test]
fn arrow_down_in_popup_focuses_first_item() {
    let (mut app, _wid) = make_interact_app();
    let menu_path =
        zbus::zvariant::OwnedObjectPath::try_from("/com/canonical/dbusmenu").unwrap();
    let tree = crate::surface::menu_popup::MenuTree {
        items: vec![
            crate::surface::menu_popup::MenuItem {
                id: "1".into(),
                label: "Open".into(),
                enabled: true,
                visible: true,
                icon_name: None,
                children: vec![],
                is_separator: false,
            },
            crate::surface::menu_popup::MenuItem {
                id: "2".into(),
                label: "Close".into(),
                enabled: true,
                visible: true,
                icon_name: None,
                children: vec![],
                is_separator: false,
            },
        ],
        anchor: (0, 0),
        screen_size: (0, 0),
    };
    let popup_wid = iced::window::Id::unique();
    app.menu_popups
        .insert(popup_wid, ("svc".into(), menu_path, tree, String::new()));
    // First arrow-down selects item "1".
    let _ = app.update(Message::FocusMoveNext(popup_wid));
    assert_eq!(
        app.menu_focused_item.get(&popup_wid).map(|s| s.as_str()),
        Some("1")
    );
    // Second arrow-down moves to item "2".
    let _ = app.update(Message::FocusMoveNext(popup_wid));
    assert_eq!(
        app.menu_focused_item.get(&popup_wid).map(|s| s.as_str()),
        Some("2")
    );
    // Third arrow-down wraps to item "1".
    let _ = app.update(Message::FocusMoveNext(popup_wid));
    assert_eq!(
        app.menu_focused_item.get(&popup_wid).map(|s| s.as_str()),
        Some("1")
    );
}

#[test]
fn arrow_up_in_popup_wraps_to_last() {
    let (mut app, _wid) = make_interact_app();
    let menu_path =
        zbus::zvariant::OwnedObjectPath::try_from("/com/canonical/dbusmenu").unwrap();
    let tree = crate::surface::menu_popup::MenuTree {
        items: vec![
            crate::surface::menu_popup::MenuItem {
                id: "1".into(),
                label: "Open".into(),
                enabled: true,
                visible: true,
                icon_name: None,
                children: vec![],
                is_separator: false,
            },
            crate::surface::menu_popup::MenuItem {
                id: "2".into(),
                label: "Close".into(),
                enabled: true,
                visible: true,
                icon_name: None,
                children: vec![],
                is_separator: false,
            },
        ],
        anchor: (0, 0),
        screen_size: (0, 0),
    };
    let popup_wid = iced::window::Id::unique();
    app.menu_popups
        .insert(popup_wid, ("svc".into(), menu_path, tree, String::new()));
    // First arrow-up selects last item "2".
    let _ = app.update(Message::FocusMovePrev(popup_wid));
    assert_eq!(
        app.menu_focused_item.get(&popup_wid).map(|s| s.as_str()),
        Some("2")
    );
}

#[test]
fn popup_skips_separators_and_disabled_items() {
    let (mut app, _wid) = make_interact_app();
    let menu_path =
        zbus::zvariant::OwnedObjectPath::try_from("/com/canonical/dbusmenu").unwrap();
    let tree = crate::surface::menu_popup::MenuTree {
        items: vec![
            crate::surface::menu_popup::MenuItem {
                id: "1".into(),
                label: "Open".into(),
                enabled: true,
                visible: true,
                icon_name: None,
                children: vec![],
                is_separator: false,
            },
            crate::surface::menu_popup::MenuItem {
                id: "0".into(),
                label: String::new(),
                enabled: false,
                visible: true,
                icon_name: None,
                children: vec![],
                is_separator: true,
            },
            crate::surface::menu_popup::MenuItem {
                id: "3".into(),
                label: "Disabled".into(),
                enabled: false,
                visible: true,
                icon_name: None,
                children: vec![],
                is_separator: false,
            },
            crate::surface::menu_popup::MenuItem {
                id: "4".into(),
                label: "Close".into(),
                enabled: true,
                visible: true,
                icon_name: None,
                children: vec![],
                is_separator: false,
            },
        ],
        anchor: (0, 0),
        screen_size: (0, 0),
    };
    let popup_wid = iced::window::Id::unique();
    app.menu_popups
        .insert(popup_wid, ("svc".into(), menu_path, tree, String::new()));
    // Arrow-down skips separator and disabled item.
    let _ = app.update(Message::FocusMoveNext(popup_wid));
    assert_eq!(
        app.menu_focused_item.get(&popup_wid).map(|s| s.as_str()),
        Some("1")
    );
    let _ = app.update(Message::FocusMoveNext(popup_wid));
    assert_eq!(
        app.menu_focused_item.get(&popup_wid).map(|s| s.as_str()),
        Some("4")
    );
}
```

- [ ] **Step 10: Run all tests**

```bash
cargo test 2>&1 | tail -5
```
Expected: all tests pass

- [ ] **Step 11: Commit**

```bash
git add crates/niribar/src/app.rs
git commit -m "feat(app): popup-aware keyboard routing and menu_focused_item navigation"
```

---

## Task 5: Clear bar focus on popup close

**Files:**
- Modify: `crates/niribar/src/app.rs`

- [ ] **Step 1: Write failing test**

Add to the `#[cfg(test)]` block in `crates/niribar/src/app.rs`:

```rust
#[test]
fn menu_closed_clears_bar_focus_on_all_surfaces() {
    let (mut app, wid) = make_interact_app();
    // Give the bar surface keyboard focus on a module.
    let _ = app.update(Message::BarFocusGained(wid));
    assert!(app.surfaces[&wid].focused_module.is_some());
    // Open a popup.
    let menu_path =
        zbus::zvariant::OwnedObjectPath::try_from("/com/canonical/dbusmenu").unwrap();
    let tree = crate::surface::menu_popup::MenuTree {
        items: vec![],
        anchor: (0, 0),
        screen_size: (0, 0),
    };
    let popup_wid = iced::window::Id::unique();
    app.menu_popups
        .insert(popup_wid, ("svc".into(), menu_path, tree, String::new()));
    app.menu_focused_item.insert(popup_wid, "1".into());
    // Close the menu.
    let _ = app.update(Message::MenuClosed);
    // Bar focus should be cleared.
    assert!(app.surfaces[&wid].focused_module.is_none());
    assert!(!app.surfaces[&wid].module_activated);
    // Menu focus state should be cleaned up.
    assert!(app.menu_focused_item.is_empty());
}
```

- [ ] **Step 2: Run test to verify it fails**

```bash
cargo test menu_closed_clears_bar_focus 2>&1 | tail -5
```
Expected: FAIL -- `focused_module` is still `Some` after `MenuClosed`

- [ ] **Step 3: Update `MenuClosed` handler to clear focus**

In the `MenuClosed` handler in `crates/niribar/src/app.rs`, after removing popups and before returning, add focus clearing:

```rust
            Message::MenuClosed => {
                // Send Event(0, "closed") to each open popup's DBusMenu before tearing down.
                let conn = self.shared.dbus.session.clone();
                let ids: Vec<iced::window::Id> = self.menu_popups.keys().copied().collect();
                for id in &ids {
                    if let Some((service, menu_path, _, _)) = self.menu_popups.get(id) {
                        if let Some(conn) = conn.clone() {
                            let service = service.clone();
                            let menu_path = menu_path.clone();
                            tokio::task::spawn(async move {
                                let _ = crate::dbus::sni_host::send_menu_event(
                                    &conn, &service, &menu_path, 0, "closed",
                                )
                                .await;
                            });
                        }
                    }
                }
                let tasks: Vec<Task<Message>> = ids
                    .into_iter()
                    .map(|id| {
                        self.menu_popups.remove(&id);
                        self.menu_popup_hovered.remove(&id);
                        self.menu_focused_item.remove(&id);
                        Task::done(Message::RemoveWindow(id))
                    })
                    .collect();
                // Clear keyboard focus on all bar surfaces.
                for s in self.surfaces.values_mut() {
                    s.focused_module = None;
                    s.module_activated = false;
                }
                Task::batch(tasks)
            }
```

- [ ] **Step 4: Also update `MenuItemSelected` handler to clear focus**

In the `MenuItemSelected` handler, add the same focus clearing after popup removal:

```rust
                // Close all popups.
                let ids: Vec<iced::window::Id> = self.menu_popups.keys().copied().collect();
                let tasks: Vec<Task<Message>> = ids
                    .into_iter()
                    .map(|id| {
                        self.menu_popups.remove(&id);
                        self.menu_popup_hovered.remove(&id);
                        self.menu_focused_item.remove(&id);
                        Task::done(Message::RemoveWindow(id))
                    })
                    .collect();
                // Clear keyboard focus on all bar surfaces.
                for s in self.surfaces.values_mut() {
                    s.focused_module = None;
                    s.module_activated = false;
                }
                Task::batch(tasks)
```

- [ ] **Step 5: Run tests**

```bash
cargo test 2>&1 | tail -5
```
Expected: all tests pass

- [ ] **Step 6: Commit**

```bash
git add crates/niribar/src/app.rs
git commit -m "feat(app): clear bar focus on popup close"
```

---

## Task 6: Pass keyboard focus to `view_menu_popup`

**Files:**
- Modify: `crates/niribar/src/surface/menu_popup.rs:36-41`
- Modify: `crates/niribar/src/app.rs` (view function, line ~1427)

- [ ] **Step 1: Add `focused_item` parameter to `view_menu_popup`**

In `crates/niribar/src/surface/menu_popup.rs`, change the function signature from:

```rust
pub fn view_menu_popup<'a>(
    tree: &'a MenuTree,
    service: &'a str,
    menu_path: &'a zbus::zvariant::OwnedObjectPath,
    window: iced::window::Id,
    hovered_item: Option<&'a str>,
) -> iced::Element<'a, crate::app::Message> {
```

to:

```rust
pub fn view_menu_popup<'a>(
    tree: &'a MenuTree,
    service: &'a str,
    menu_path: &'a zbus::zvariant::OwnedObjectPath,
    window: iced::window::Id,
    hovered_item: Option<&'a str>,
    focused_item: Option<&'a str>,
) -> iced::Element<'a, crate::app::Message> {
```

Change the highlight logic inside `view_menu_popup`. Find the line:

```rust
                let is_hovered = item.enabled
                    && hovered_item.map(|h| h == item.id.as_str()).unwrap_or(false);
```

Replace with:

```rust
                let is_highlighted = item.enabled
                    && hovered_item
                        .or(focused_item)
                        .map(|h| h == item.id.as_str())
                        .unwrap_or(false);
```

And update the reference to `is_hovered` in the style closure to use `is_highlighted`:

```rust
                    .style(move |_| iced::widget::container::Style {
                        background: if is_highlighted {
```

- [ ] **Step 2: Update the call site in `app.rs` view function**

In `crates/niribar/src/app.rs`, find the call to `view_menu_popup` (line ~1427):

```rust
            let hovered_item = self.menu_popup_hovered.get(&window).map(|s| s.as_str());
            let menu = view_menu_popup(tree, service.as_str(), menu_path, window, hovered_item);
```

Change to:

```rust
            let hovered_item = self.menu_popup_hovered.get(&window).map(|s| s.as_str());
            let focused_item = self.menu_focused_item.get(&window).map(|s| s.as_str());
            let menu = view_menu_popup(tree, service.as_str(), menu_path, window, hovered_item, focused_item);
```

- [ ] **Step 3: Build and test**

```bash
cargo build 2>&1 | tail -5
cargo test 2>&1 | tail -5
```
Expected: zero warnings, all tests pass

- [ ] **Step 4: Commit**

```bash
git add crates/niribar/src/surface/menu_popup.rs crates/niribar/src/app.rs
git commit -m "feat(menu): pass keyboard focus to view_menu_popup for highlight rendering"
```

---

## Self-Review

**Spec coverage:**

| Spec requirement | Task |
|-----------------|------|
| Add `Activated` to `InteractionEvent` | Task 1 |
| Enter maps to `KeyboardActivate`, Space to `KeyboardSecondaryActivate` | Task 2 |
| `KeyboardSecondaryActivate` handler calls `on_interaction(Activated)` | Task 2 |
| `TrayIconModule` handles `Activated` -> `TrayMenuRequest` | Task 3 |
| `menu_focused_item` state in `App` | Task 4 |
| Popup-aware routing in `EscapePressed` | Task 4 |
| Popup-aware routing in `FocusMoveNext`/`FocusMovePrev` | Task 4 |
| Popup-aware routing in `KeyboardActivate` (Enter selects item) | Task 4 |
| Arrow Down/Up wrap around | Task 4 |
| Separators and disabled items skipped | Task 4 |
| No item focused initially, first arrow selects first/last | Task 4 |
| Enter with no item focused does nothing | Task 4 |
| Escape closes popup | Task 4 |
| Focus cleared on all surfaces after popup close | Task 5 |
| `menu_focused_item` cleaned up on close | Task 5 |
| Mouse hover takes priority over keyboard focus in rendering | Task 6 |
| `view_menu_popup` accepts `focused_item` parameter | Task 6 |

**Placeholder scan:** No TBDs, TODOs, or incomplete sections found.

**Type consistency:** `InteractionEvent::Activated` used consistently in Tasks 1, 2, 3. `menu_focused_item` field name consistent across Tasks 4, 5, 6. `KeyboardSecondaryActivate` message name consistent in Task 2.
