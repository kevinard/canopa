# Panel Menu Session Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Deepen Panel Menu selection behavior behind a `PanelMenuSession` module so `App` no longer coordinates raw search, focus, source, cache, submenu, and Dmenu selection state directly.

**Architecture:** Add a new `crates/niribar/src/app/panel_menu_session.rs` module that owns the live **Panel Menu Session**: source-specific entry projection, Panel Menu Search, focus, submenu navigation, and activation outcome calculation. `App` remains the adapter for overlay windows, process launch, configured command execution, and IPC responder completion.

**Tech Stack:** Rust, iced/iced_layershell, existing `PanelEvent` and `PanelContent` flow, existing `panel_search` fuzzy matcher, existing `cargo test` suite.

---

## File Structure

- Create `crates/niribar/src/app/panel_menu_session.rs`
  - Owns `PanelMenuSession`, `PanelMenuSource`, `PanelMenuEntry`, `PanelMenuActivation`, and `PanelMenuSnapshot`.
  - Contains moved pure functions for configured menu entries, installed application entries, Dmenu entries, and activation target resolution.
  - Contains unit tests for search, focus, submenu navigation, Dmenu custom values, installed-app activation outcomes, and configured menu action outcomes.
- Modify `crates/niribar/src/app.rs`
  - Add `mod panel_menu_session;`.
- Modify `crates/niribar/src/app/panel.rs`
  - Replace raw source/session fields in `PanelSystem` with `session: Option<PanelMenuSession>`.
  - Keep overlay/window concerns: closing window tracking, search focus retries, scroll position, and IPC responder.
  - Apply `PanelMenuActivation` outcomes in `App`.
- Modify `crates/niribar/src/app/render.rs`
  - Build panel render models from `PanelMenuSession::snapshot()` instead of inspecting Dmenu/config/installed-app source details.
- Modify `crates/niribar/src/app/view.rs`
  - Keep widget rendering mostly unchanged, but consume snapshot entry data rather than source-specific structs where needed.

---

### Task 1: Add Panel Menu Session Types

**Files:**
- Create: `crates/niribar/src/app/panel_menu_session.rs`
- Modify: `crates/niribar/src/app.rs`

- [ ] **Step 1: Wire the module into the app tree**

In `crates/niribar/src/app.rs`, add the module next to the current panel modules:

```rust
mod panel;
mod panel_menu_session;
mod panel_search;
```

- [ ] **Step 2: Create the initial session module**

Create `crates/niribar/src/app/panel_menu_session.rs` with:

```rust
use super::panel_search::{self, SearchField, SearchMatch, SearchText};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum PanelMenuActivation {
    RunPanelAction(crate::config::PanelAction),
    LaunchInstalledApp(crate::desktop_apps::DesktopApplicationId),
    CompleteDmenu(crate::ipc::protocol::IpcResponse),
    ClosePanel,
    None,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum PanelMenuEntryTarget {
    StaticPath(Vec<usize>),
    InstalledApp(crate::desktop_apps::DesktopApplicationId),
    DmenuValue(String),
    DmenuCustomValue(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct PanelMenuEntry {
    pub(super) label: String,
    pub(super) icon: Option<String>,
    pub(super) path: Vec<usize>,
    pub(super) target: PanelMenuEntryTarget,
    pub(super) is_submenu: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct PanelMenuSnapshot {
    pub(super) prompt: Option<String>,
    pub(super) search_query: String,
    pub(super) focused_index: Option<usize>,
    pub(super) show_icons: bool,
    pub(super) entries: Vec<PanelMenuEntry>,
}

#[derive(Debug, Clone)]
pub(super) enum PanelMenuSource {
    Configured(crate::config::MenuConfig),
    InstalledApps {
        prompt: Option<String>,
        show_icons: bool,
        apps: Vec<crate::desktop_apps::DesktopApplication>,
    },
    Dmenu(crate::ipc::protocol::DmenuRequest),
}

#[derive(Debug, Clone)]
pub(super) struct PanelMenuSession {
    source: PanelMenuSource,
    search_query: String,
    focused_index: Option<usize>,
    menu_path: Vec<usize>,
}

impl PanelMenuSession {
    pub(super) fn open(source: PanelMenuSource) -> Self {
        Self {
            source,
            search_query: String::new(),
            focused_index: Some(0),
            menu_path: Vec::new(),
        }
    }

    pub(super) fn search_query(&self) -> &str {
        &self.search_query
    }

    pub(super) fn set_search_query(&mut self, query: String) {
        self.search_query = query;
        self.focused_index = Some(0);
    }

    pub(super) fn clear_search(&mut self) -> bool {
        if self.search_query.is_empty() {
            return false;
        }
        self.search_query.clear();
        self.focused_index = Some(0);
        true
    }

    pub(super) fn focus_index(&mut self, index: usize) {
        self.focused_index = Some(index);
    }

    pub(super) fn focused_index(&self) -> Option<usize> {
        self.focused_index
    }

    pub(super) fn move_focus(&mut self, delta: isize) {
        let entries = self.entries();
        if entries.is_empty() {
            self.focused_index = None;
            return;
        }
        let current = self.focused_index.unwrap_or(0).min(entries.len() - 1);
        let next = if delta < 0 {
            current.saturating_sub(delta.unsigned_abs())
        } else {
            (current + delta as usize).min(entries.len() - 1)
        };
        self.focused_index = Some(next);
    }

    pub(super) fn back(&mut self) -> bool {
        if self.clear_search() {
            return true;
        }
        if self.menu_path.is_empty() {
            return false;
        }
        self.menu_path.pop();
        self.focused_index = Some(0);
        true
    }

    pub(super) fn snapshot(&self) -> PanelMenuSnapshot {
        PanelMenuSnapshot {
            prompt: self.prompt(),
            search_query: self.search_query.clone(),
            focused_index: self.focused_index,
            show_icons: self.show_icons(),
            entries: self.entries(),
        }
    }

    pub(super) fn activate_focused(&mut self) -> PanelMenuActivation {
        let Some(index) = self.focused_index else {
            return PanelMenuActivation::None;
        };
        self.activate_index(index)
    }

    pub(super) fn activate_index(&mut self, index: usize) -> PanelMenuActivation {
        let Some(entry) = self.entries().get(index).cloned() else {
            return PanelMenuActivation::None;
        };

        match entry.target {
            PanelMenuEntryTarget::StaticPath(path) => self.activate_static_path(path),
            PanelMenuEntryTarget::InstalledApp(id) => PanelMenuActivation::LaunchInstalledApp(id),
            PanelMenuEntryTarget::DmenuValue(value) => {
                PanelMenuActivation::CompleteDmenu(crate::ipc::protocol::IpcResponse::Selected {
                    value,
                })
            }
            PanelMenuEntryTarget::DmenuCustomValue(value) => {
                PanelMenuActivation::CompleteDmenu(crate::ipc::protocol::IpcResponse::Selected {
                    value,
                })
            }
        }
    }

    fn prompt(&self) -> Option<String> {
        match &self.source {
            PanelMenuSource::Configured(menu) => menu.prompt.clone(),
            PanelMenuSource::InstalledApps { prompt, .. } => prompt.clone(),
            PanelMenuSource::Dmenu(request) => request.prompt.clone(),
        }
    }

    fn show_icons(&self) -> bool {
        match &self.source {
            PanelMenuSource::Configured(menu) => menu.show_icons,
            PanelMenuSource::InstalledApps { show_icons, .. } => *show_icons,
            PanelMenuSource::Dmenu(request) => dmenu_request_show_icons(request),
        }
    }

    fn entries(&self) -> Vec<PanelMenuEntry> {
        match &self.source {
            PanelMenuSource::Configured(menu) if self.search_query.is_empty() => {
                current_menu_entries(menu, &self.menu_path)
            }
            PanelMenuSource::Configured(menu) => search_menu_entries(menu, &self.search_query),
            PanelMenuSource::InstalledApps { apps, .. } => {
                installed_apps_entry_views(apps, &self.search_query)
            }
            PanelMenuSource::Dmenu(request) => dmenu_entry_views(request, &self.search_query),
        }
    }

    fn activate_static_path(&mut self, path: Vec<usize>) -> PanelMenuActivation {
        let PanelMenuSource::Configured(menu) = &self.source else {
            return PanelMenuActivation::None;
        };
        let Some(item) = menu_item_at_path(&menu.items, &path) else {
            return PanelMenuActivation::None;
        };
        if !item.children.is_empty() && self.search_query.is_empty() {
            self.menu_path = path;
            self.focused_index = Some(0);
            return PanelMenuActivation::None;
        }
        match &item.action {
            Some(action) => PanelMenuActivation::RunPanelAction(action.clone()),
            None => PanelMenuActivation::None,
        }
    }
}

fn menu_item_at_path<'a>(
    items: &'a [crate::config::MenuItemConfig],
    path: &[usize],
) -> Option<&'a crate::config::MenuItemConfig> {
    let (first, rest) = path.split_first()?;
    let item = items.get(*first)?;
    if rest.is_empty() {
        Some(item)
    } else {
        menu_item_at_path(&item.children, rest)
    }
}

fn current_menu_entries(
    menu: &crate::config::MenuConfig,
    path: &[usize],
) -> Vec<PanelMenuEntry> {
    let items = if path.is_empty() {
        &menu.items
    } else {
        menu_item_at_path(&menu.items, path)
            .map(|item| item.children.as_slice())
            .unwrap_or(&[])
    };
    items
        .iter()
        .enumerate()
        .map(|(index, item)| {
            let mut item_path = path.to_vec();
            item_path.push(index);
            PanelMenuEntry {
                label: item.label.clone(),
                icon: item.icon.clone(),
                target: PanelMenuEntryTarget::StaticPath(item_path.clone()),
                path: item_path,
                is_submenu: !item.children.is_empty(),
            }
        })
        .collect()
}

fn search_menu_entries(menu: &crate::config::MenuConfig, query: &str) -> Vec<PanelMenuEntry> {
    if panel_search::is_blank_query(query) {
        return current_menu_entries(menu, &[]);
    }
    let mut results = Vec::new();
    collect_search_menu_entries(
        &menu.items,
        query,
        &mut Vec::new(),
        &mut Vec::new(),
        &mut 0,
        &mut results,
    );
    panel_search::rank_matches(results)
}

fn collect_search_menu_entries(
    items: &[crate::config::MenuItemConfig],
    query: &str,
    path: &mut Vec<usize>,
    breadcrumbs: &mut Vec<String>,
    source_index: &mut usize,
    results: &mut Vec<SearchMatch<PanelMenuEntry>>,
) {
    for (index, item) in items.iter().enumerate() {
        path.push(index);
        let current_source_index = *source_index;
        *source_index += 1;
        let mut fields = vec![SearchText {
            field: SearchField::Label,
            text: item.label.as_str(),
        }];
        for breadcrumb in breadcrumbs.iter() {
            fields.push(SearchText {
                field: SearchField::Breadcrumb,
                text: breadcrumb.as_str(),
            });
        }
        if let Some(score) = panel_search::best_match(&fields, query) {
            results.push(SearchMatch {
                item: PanelMenuEntry {
                    label: if breadcrumbs.is_empty() {
                        item.label.clone()
                    } else {
                        format!("{} / {}", breadcrumbs.join(" / "), item.label)
                    },
                    icon: item.icon.clone(),
                    target: PanelMenuEntryTarget::StaticPath(path.clone()),
                    path: path.clone(),
                    is_submenu: !item.children.is_empty(),
                },
                score,
                source_index: current_source_index,
            });
        }
        breadcrumbs.push(item.label.clone());
        collect_search_menu_entries(&item.children, query, path, breadcrumbs, source_index, results);
        breadcrumbs.pop();
        path.pop();
    }
}

fn installed_apps_entry_views(
    apps: &[crate::desktop_apps::DesktopApplication],
    query: &str,
) -> Vec<PanelMenuEntry> {
    let mut matches = Vec::new();
    for (index, app) in apps.iter().enumerate() {
        let icon = app.icon.clone().filter(|icon| crate::icon_theme::is_icon_name(icon));
        let view = PanelMenuEntry {
            label: app.name.clone(),
            icon,
            path: vec![index],
            target: PanelMenuEntryTarget::InstalledApp(app.id.clone()),
            is_submenu: false,
        };
        let keywords = app.keywords.join(" ");
        let fields = [
            SearchText {
                field: SearchField::Label,
                text: app.name.as_str(),
            },
            SearchText {
                field: SearchField::Secondary,
                text: app.comment.as_deref().unwrap_or(""),
            },
            SearchText {
                field: SearchField::Secondary,
                text: app.generic_name.as_deref().unwrap_or(""),
            },
            SearchText {
                field: SearchField::Secondary,
                text: keywords.as_str(),
            },
        ];
        if let Some(score) = panel_search::best_match(&fields, query) {
            matches.push(SearchMatch {
                item: view,
                score,
                source_index: index,
            });
        }
    }
    panel_search::rank_matches(matches)
}

fn dmenu_entry_views(
    request: &crate::ipc::protocol::DmenuRequest,
    query: &str,
) -> Vec<PanelMenuEntry> {
    let exact_match = request
        .entries
        .iter()
        .any(|entry| entry.display_label() == query || entry.selected_value() == query);

    let mut matches = Vec::new();
    for (index, entry) in request.entries.iter().enumerate() {
        let view = PanelMenuEntry {
            label: entry.display_label().to_string(),
            icon: entry.icon.clone(),
            path: vec![index],
            target: PanelMenuEntryTarget::DmenuValue(entry.selected_value().to_string()),
            is_submenu: false,
        };

        if panel_search::is_blank_query(query) {
            matches.push(SearchMatch {
                item: view,
                score: panel_search::best_match(
                    &[SearchText {
                        field: SearchField::Label,
                        text: entry.display_label(),
                    }],
                    "",
                )
                .expect("blank query matches"),
                source_index: index,
            });
            continue;
        }

        let keywords = entry.keywords.join(" ");
        let fields = [
            SearchText {
                field: SearchField::Label,
                text: entry.display_label(),
            },
            SearchText {
                field: SearchField::Secondary,
                text: keywords.as_str(),
            },
        ];

        if let Some(score) = panel_search::best_match(&fields, query) {
            matches.push(SearchMatch {
                item: view,
                score,
                source_index: index,
            });
        }
    }

    let mut entries = if panel_search::is_blank_query(query) {
        matches.into_iter().map(|matched| matched.item).collect()
    } else {
        panel_search::rank_matches(matches)
    };

    if request.allow_custom && !panel_search::is_blank_query(query) && !exact_match {
        entries.push(PanelMenuEntry {
            label: query.to_string(),
            icon: None,
            path: Vec::new(),
            target: PanelMenuEntryTarget::DmenuCustomValue(query.to_string()),
            is_submenu: false,
        });
    }

    entries
}

fn dmenu_request_show_icons(request: &crate::ipc::protocol::DmenuRequest) -> bool {
    request.entries.iter().any(|entry| entry.icon.is_some())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dmenu_request() -> crate::ipc::protocol::DmenuRequest {
        crate::ipc::protocol::DmenuRequest {
            prompt: Some("Run".into()),
            entries: vec![
                crate::ipc::protocol::DmenuEntry::plain("Firefox"),
                crate::ipc::protocol::DmenuEntry::plain("Kitty"),
            ],
            allow_custom: true,
        }
    }

    #[test]
    fn dmenu_snapshot_preserves_blank_query_source_order() {
        let session = PanelMenuSession::open(PanelMenuSource::Dmenu(dmenu_request()));

        let snapshot = session.snapshot();

        assert_eq!(snapshot.prompt.as_deref(), Some("Run"));
        assert_eq!(snapshot.entries[0].label, "Firefox");
        assert_eq!(snapshot.entries[1].label, "Kitty");
    }

    #[test]
    fn dmenu_activation_returns_selected_response() {
        let mut session = PanelMenuSession::open(PanelMenuSource::Dmenu(dmenu_request()));

        let activation = session.activate_index(1);

        assert_eq!(
            activation,
            PanelMenuActivation::CompleteDmenu(crate::ipc::protocol::IpcResponse::Selected {
                value: "Kitty".into()
            })
        );
    }
}
```

- [ ] **Step 3: Run focused compilation**

Run:

```bash
cargo test app::panel_menu_session --lib
```

Expected: compilation errors if `DesktopApplicationId` is not a type alias. If so, replace `crate::desktop_apps::DesktopApplicationId` with `String` in `PanelMenuActivation::LaunchInstalledApp` and `PanelMenuEntryTarget::InstalledApp`.

- [ ] **Step 4: Run focused tests again**

Run:

```bash
cargo test app::panel_menu_session --lib
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
rtk git add crates/niribar/src/app.rs crates/niribar/src/app/panel_menu_session.rs
rtk git commit -m "refactor: add panel menu session module"
```

---

### Task 2: Move Existing Entry Projection Tests

**Files:**
- Modify: `crates/niribar/src/app/panel.rs`
- Modify: `crates/niribar/src/app/panel_menu_session.rs`

- [ ] **Step 1: Move Dmenu tests from panel to session**

Move the existing tests named `dmenu_entries_use_input_as_label_and_value`, `dmenu_search_filters_entries_case_insensitively`, `dmenu_custom_value_entry_appears_when_query_has_no_exact_match`, `dmenu_no_custom_does_not_create_custom_entry`, `dmenu_rich_entries_use_label_icon_keywords_and_value`, `dmenu_search_uses_fuzzy_ranking`, `dmenu_custom_value_appears_after_fuzzy_matches`, `dmenu_blank_query_preserves_duplicate_source_order`, `dmenu_rich_search_does_not_match_opaque_value_when_label_exists`, and `dmenu_value_only_rich_entry_matches_and_displays_value` from `crates/niribar/src/app/panel.rs` into `crates/niribar/src/app/panel_menu_session.rs`.

Update each test to create:

```rust
let mut session = PanelMenuSession::open(PanelMenuSource::Dmenu(request));
session.set_search_query("query".into());
let entries = session.snapshot().entries;
```

instead of calling `dmenu_entry_views(&request, "query")`.

- [ ] **Step 2: Move Installed Desktop Application projection tests**

Move the installed-app entry tests from `crates/niribar/src/app/panel.rs` that directly validate `installed_apps_entry_views` behavior into `crates/niribar/src/app/panel_menu_session.rs`.

Use this shape:

```rust
let mut session = PanelMenuSession::open(PanelMenuSource::InstalledApps {
    prompt: Some("Apps".into()),
    show_icons: true,
    apps,
});
session.set_search_query("browser".into());
let snapshot = session.snapshot();
```

- [ ] **Step 3: Keep integration-style App tests in panel**

Do not move tests that assert `App` closes Panels, completes responders, launches applications, or applies overlay effects. Those remain in `crates/niribar/src/app/panel.rs` because they cross the App adapter seam.

- [ ] **Step 4: Run moved test subset**

Run:

```bash
cargo test app::panel_menu_session --lib
```

Expected: PASS.

- [ ] **Step 5: Run panel tests to catch missed references**

Run:

```bash
cargo test app::panel --lib
```

Expected: PASS or compile errors only from now-private moved helpers. Fix by updating remaining panel tests to use `PanelMenuSession` snapshots or by leaving adapter-specific helpers in `panel.rs`.

- [ ] **Step 6: Commit**

```bash
rtk git add crates/niribar/src/app/panel.rs crates/niribar/src/app/panel_menu_session.rs
rtk git commit -m "test: move panel menu projection tests to session"
```

---

### Task 3: Replace PanelSystem Raw Selection State

**Files:**
- Modify: `crates/niribar/src/app/panel.rs`
- Modify: `crates/niribar/src/app/render.rs`

- [ ] **Step 1: Change PanelSystem fields**

In `crates/niribar/src/app/panel.rs`, replace these fields:

```rust
pub(super) search_query: String,
pub(super) focused_index: Option<usize>,
pub(super) menu_path: Vec<usize>,
pub(super) installed_apps_snapshot: Vec<crate::desktop_apps::DesktopApplication>,
pub(super) dmenu_request: Option<crate::ipc::protocol::DmenuRequest>,
installed_apps_entry_cache: InstalledAppsEntryCache,
```

with:

```rust
pub(super) session: Option<super::panel_menu_session::PanelMenuSession>,
```

Keep these fields in `PanelSystem`:

```rust
pub(super) menu_scroll_y: f32,
menu_viewport_height: f32,
search_focus_pending: bool,
pub(super) dmenu_responder: Option<super::messages::IpcResponder>,
#[cfg(test)]
pub(super) installed_apps_for_test: Option<Vec<crate::desktop_apps::DesktopApplication>>,
#[cfg(test)]
pub(super) launched_installed_app_ids_for_test: Vec<String>,
closing_windows: HashSet<iced::window::Id>,
```

- [ ] **Step 2: Update clear behavior**

Change `PanelSystem::clear` to:

```rust
pub(super) fn clear(&mut self) {
    self.session = None;
    self.menu_scroll_y = 0.0;
    self.menu_viewport_height = 0.0;
    self.search_focus_pending = false;
    self.dmenu_responder = None;
}
```

- [ ] **Step 3: Update escape search clearing**

Replace `escape_clears_search` with:

```rust
pub(super) fn escape_clears_search(&mut self) -> bool {
    self.session
        .as_mut()
        .map(super::panel_menu_session::PanelMenuSession::clear_search)
        .unwrap_or(false)
}
```

- [ ] **Step 4: Add helper accessors in panel.rs**

Add helpers on `App`:

```rust
fn panel_menu_snapshot(
    &self,
) -> Option<super::panel_menu_session::PanelMenuSnapshot> {
    self.panel.session.as_ref().map(|session| session.snapshot())
}

fn active_panel_menu_entries(
    &self,
) -> Vec<super::panel_menu_session::PanelMenuEntry> {
    self.panel
        .session
        .as_ref()
        .map(|session| session.snapshot().entries)
        .unwrap_or_default()
}
```

- [ ] **Step 5: Run compile to identify raw state references**

Run:

```bash
cargo test --lib
```

Expected: compile failures in `panel.rs`, `render.rs`, and possibly `view.rs` where code still references removed fields.

- [ ] **Step 6: Commit only after compile reaches green in later tasks**

Do not commit this task until Task 4 and Task 5 complete; this task intentionally creates temporary compile failures.

---

### Task 4: Open Sessions From Panel Events

**Files:**
- Modify: `crates/niribar/src/app/panel.rs`

- [ ] **Step 1: Build a session for configured Panel Menus**

In `handle_panel_open`, when `tree.content` is `PanelContent::Menu(name)`, find the `MenuConfig` and set:

```rust
self.panel.session = Some(super::panel_menu_session::PanelMenuSession::open(
    super::panel_menu_session::PanelMenuSource::Configured(menu.clone()),
));
```

For installed-app dynamic sources, load the open snapshot first:

```rust
let apps = self.load_installed_apps_for_panel();
self.panel.session = Some(super::panel_menu_session::PanelMenuSession::open(
    super::panel_menu_session::PanelMenuSource::InstalledApps {
        prompt: menu.prompt.clone(),
        show_icons: menu.show_icons,
        apps,
    },
));
```

- [ ] **Step 2: Build a session for Dmenu Selection Requests**

In `handle_dmenu_selection_request`, after closing existing Panels, replace:

```rust
self.panel.dmenu_request = Some(request);
self.panel.focused_index = Some(0);
```

with:

```rust
self.panel.session = Some(super::panel_menu_session::PanelMenuSession::open(
    super::panel_menu_session::PanelMenuSource::Dmenu(request),
));
```

Keep:

```rust
self.panel.dmenu_responder = Some(responder);
```

- [ ] **Step 3: Update active Dmenu checks**

Replace checks of `self.panel.dmenu_request.is_some()` with:

```rust
self.panel.dmenu_responder.is_some()
```

This preserves the “one active Dmenu Selection Request” rule without storing transport state inside the session.

- [ ] **Step 4: Cancel Dmenu through responder only**

In `close_all_panels` and `close_all_untracked_panels_with_restore`, keep:

```rust
if self.panel.dmenu_responder.is_some() {
    self.complete_dmenu_request(crate::ipc::protocol::IpcResponse::Cancelled);
}
```

Then clear the session through `self.panel.clear()`.

- [ ] **Step 5: Update Dmenu completion**

Change `complete_dmenu_request` to:

```rust
pub(super) fn complete_dmenu_request(&mut self, response: crate::ipc::protocol::IpcResponse) {
    if let Some(responder) = self.panel.dmenu_responder.take() {
        send_ipc_response(responder, response);
    }
}
```

- [ ] **Step 6: Run panel compile**

Run:

```bash
cargo test app::panel --lib
```

Expected: remaining compile failures only from activation and rendering paths that still reference old entry helper types.

---

### Task 5: Apply Session Activation Outcomes

**Files:**
- Modify: `crates/niribar/src/app/panel.rs`

- [ ] **Step 1: Replace direct target matching in activation**

In `handle_panel_activate`, replace logic that matches `MenuEntryTarget` with:

```rust
let activation = self
    .panel
    .session
    .as_mut()
    .map(|session| session.activate_focused())
    .unwrap_or(super::panel_menu_session::PanelMenuActivation::None);

self.apply_panel_menu_activation(activation)
```

- [ ] **Step 2: Add outcome adapter**

Add this method to `impl App` in `crates/niribar/src/app/panel.rs`:

```rust
fn apply_panel_menu_activation(
    &mut self,
    activation: super::panel_menu_session::PanelMenuActivation,
) -> Task<Message> {
    match activation {
        super::panel_menu_session::PanelMenuActivation::RunPanelAction(action) => {
            let task = self.dispatch_panel_action_from_active_panel(action);
            Task::batch([task, Task::batch(self.close_all_panels())])
        }
        super::panel_menu_session::PanelMenuActivation::LaunchInstalledApp(id) => {
            self.launch_installed_app_by_id(&id);
            Task::batch(self.close_all_panels())
        }
        super::panel_menu_session::PanelMenuActivation::CompleteDmenu(response) => {
            self.complete_dmenu_request(response);
            Task::batch(self.close_all_panels())
        }
        super::panel_menu_session::PanelMenuActivation::ClosePanel => {
            Task::batch(self.close_all_panels())
        }
        super::panel_menu_session::PanelMenuActivation::None => Task::none(),
    }
}
```

If `dispatch_panel_action_from_active_panel` does not exist, implement it with the same origin-window/origin behavior currently used by configured Panel Menu item activation. If the current code dispatches directly from a selected item, move that exact code into this helper.

- [ ] **Step 3: Add installed-app launch helper**

Move the existing installed-app launch logic into:

```rust
fn launch_installed_app_by_id(&mut self, id: &str) {
    #[cfg(test)]
    {
        self.panel.launched_installed_app_ids_for_test.push(id.to_string());
        return;
    }

    let Some(app) = self
        .panel
        .session
        .as_ref()
        .and_then(|session| session.snapshot().entries.into_iter().find_map(|entry| {
            match entry.target {
                super::panel_menu_session::PanelMenuEntryTarget::InstalledApp(entry_id)
                    if entry_id == id =>
                {
                    Some(entry_id)
                }
                _ => None,
            }
        }))
    else {
        return;
    };

    let _ = crate::desktop_apps::launch_by_id(&app);
}
```

If `desktop_apps::launch_by_id` does not exist, keep the existing launch function and adapt the helper to the current launch mechanism. The important rule is that `PanelMenuSession` returns the Desktop Application ID and `App` performs the process-launch side effect.

- [ ] **Step 4: Run activation tests**

Run:

```bash
cargo test app::panel --lib
```

Expected: PASS after adapting helper names to existing launch/dispatch functions.

- [ ] **Step 5: Commit Task 3 through Task 5 together**

```bash
rtk git add crates/niribar/src/app/panel.rs crates/niribar/src/app/render.rs crates/niribar/src/app/panel_menu_session.rs
rtk git commit -m "refactor: route panel menu activation through session"
```

---

### Task 6: Render From Session Snapshot

**Files:**
- Modify: `crates/niribar/src/app/render.rs`
- Modify: `crates/niribar/src/app/view.rs`
- Modify: `crates/niribar/src/app/panel.rs`

- [ ] **Step 1: Replace source-specific render checks**

In `crates/niribar/src/app/render.rs`, replace `PanelContent::DmenuRequest` and installed-app source checks that reach into `self.panel.dmenu_request`, `self.config.menus`, or `self.panel.installed_apps_snapshot` with:

```rust
let snapshot = self.panel_menu_snapshot()?;
```

Use:

```rust
snapshot.entries
snapshot.prompt
snapshot.show_icons
snapshot.focused_index
snapshot.search_query
```

for the render model.

- [ ] **Step 2: Keep scroll position outside the session**

Continue to populate render model scroll fields from:

```rust
self.panel.menu_scroll_y
```

Do not move scroll into `PanelMenuSession` in this refactor.

- [ ] **Step 3: Update view type imports**

In `crates/niribar/src/app/view.rs`, replace imports/usages of:

```rust
super::panel::MenuEntryView
super::panel::MenuEntryTarget
```

with:

```rust
super::panel_menu_session::PanelMenuEntry
super::panel_menu_session::PanelMenuEntryTarget
```

- [ ] **Step 4: Run render/view tests**

Run:

```bash
cargo test app::render app::view --lib
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
rtk git add crates/niribar/src/app/render.rs crates/niribar/src/app/view.rs crates/niribar/src/app/panel.rs
rtk git commit -m "refactor: render panel menus from session snapshots"
```

---

### Task 7: Delete Old Shallow Helpers From panel.rs

**Files:**
- Modify: `crates/niribar/src/app/panel.rs`
- Modify: `crates/niribar/src/app/panel_menu_session.rs`

- [ ] **Step 1: Delete old types from panel.rs**

Remove from `crates/niribar/src/app/panel.rs`:

```rust
InstalledAppsEntryCache
MenuEntryTarget
MenuEntryView
```

- [ ] **Step 2: Delete old pure helper functions from panel.rs**

Remove from `crates/niribar/src/app/panel.rs`:

```rust
current_menu_entries
search_menu_entries
dmenu_entry_views
dmenu_request_show_icons
installed_apps_entry_views
menu_item_at_path
collect_search_menu_entries
```

Only remove helpers after their equivalents are compiled and tested in `panel_menu_session.rs`.

- [ ] **Step 3: Run search for leaked old names**

Run:

```bash
rg "MenuEntryView|MenuEntryTarget|dmenu_entry_views|installed_apps_entry_views|current_menu_entries|search_menu_entries|installed_apps_entry_cache|dmenu_request|menu_path|focused_index|search_query" crates/niribar/src/app
```

Expected:
- `MenuEntryView`, `MenuEntryTarget`, old helper names: no matches outside `panel_menu_session.rs`.
- `search_query`, `focused_index`: matches only through session snapshots or non-menu UI models.
- `dmenu_request`: no `PanelSystem` field matches.

- [ ] **Step 4: Run full library tests**

Run:

```bash
cargo test --lib
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
rtk git add crates/niribar/src/app/panel.rs crates/niribar/src/app/panel_menu_session.rs
rtk git commit -m "refactor: remove panel menu selection state from panel system"
```

---

### Task 8: Final Verification

**Files:**
- Verify only; no planned edits.

- [ ] **Step 1: Run full test suite**

Run:

```bash
cargo test
```

Expected: PASS.

- [ ] **Step 2: Run formatting**

Run:

```bash
cargo fmt --check
```

Expected: PASS. If it fails, run `cargo fmt`, then rerun `cargo fmt --check`.

- [ ] **Step 3: Run final architecture grep**

Run:

```bash
rg "dmenu_responder|closing_windows|search_focus_pending|menu_scroll_y|PanelMenuSession|PanelMenuActivation" crates/niribar/src/app
```

Expected:
- `dmenu_responder`, `closing_windows`, `search_focus_pending`, and `menu_scroll_y` remain in `panel.rs`.
- `PanelMenuSession` and `PanelMenuActivation` are used by `panel.rs`, `render.rs`, and `panel_menu_session.rs`.
- No renderer or view code inspects `DmenuRequest` directly.

- [ ] **Step 4: Commit verification fixes if any**

```bash
rtk git add crates/niribar/src/app crates/niribar/src/app.rs
rtk git commit -m "chore: finalize panel menu session refactor"
```

Only make this commit if formatting or verification required changes.

---

## Self-Review

- Spec coverage: The plan implements the agreed **Panel Menu Session** seam, keeps overlay windows/IPC/process launch outside, returns external activation outcomes, and updates rendering to consume snapshots.
- Placeholder scan: The plan includes concrete file paths, exact commands, expected outcomes, and code shapes. Where exact existing helper names are uncertain, the plan says to preserve current behavior and adapt to the existing launch/dispatch function names.
- Type consistency: `PanelMenuSession`, `PanelMenuSource`, `PanelMenuSnapshot`, `PanelMenuEntry`, `PanelMenuEntryTarget`, and `PanelMenuActivation` are introduced in Task 1 and used consistently in later tasks.
