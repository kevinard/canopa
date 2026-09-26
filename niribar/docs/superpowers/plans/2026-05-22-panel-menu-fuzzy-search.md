# Panel Menu Fuzzy Search Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace current substring-based **Panel Menu Search** with default ranked fuzzy search across configured panel menus, installed-app sources, and dmenu panel menu sources.

**Architecture:** Add a small Niribar-owned fuzzy matcher in `src/app/panel_search.rs`, then make `src/app/panel.rs` build ranked search candidates for each existing menu source. Keep empty-query behavior source-ordered, preserve dmenu value semantics, and use source order as the final tie-breaker.

**Tech Stack:** Rust, existing inline unit tests in `src/app/panel.rs`, no new external fuzzy-search dependency.

---

## File Structure

- Create `src/app/panel_search.rs`: pure fuzzy search/scoring helpers. It owns tokenization, ordered-subsequence matching, field priority, scoring, and stable sorting by match quality.
- Modify `src/app.rs`: add `mod panel_search;` beside `mod panel;`.
- Modify `src/app/panel.rs`: replace ad hoc substring filters in `dmenu_entry_views`, `search_menu_entries`, and `installed_app_entries` with `panel_search` calls.
- Test in `src/app/panel.rs`: add focused tests for configured menus, installed apps, and dmenu behavior. Existing tests remain but expected ordering changes where custom dmenu values move below fuzzy matches.
- Documentation already updated in `CONTEXT.md`; no further docs required unless implementation reveals a domain contradiction.

---

### Task 1: Add Pure Fuzzy Matcher

**Files:**
- Create: `src/app/panel_search.rs`
- Modify: `src/app.rs`

- [x] **Step 1: Write the matcher module with unit tests**

Create `src/app/panel_search.rs`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum SearchField {
    Label = 0,
    Secondary = 1,
    Breadcrumb = 2,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct SearchScore {
    field: SearchField,
    score: i32,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct SearchText<'a> {
    pub(super) field: SearchField,
    pub(super) text: &'a str,
}

#[derive(Debug, Clone)]
pub(super) struct SearchMatch<T> {
    pub(super) item: T,
    pub(super) score: SearchScore,
    pub(super) source_index: usize,
}

pub(super) fn query_tokens(query: &str) -> Vec<String> {
    query
        .split_whitespace()
        .map(str::to_ascii_lowercase)
        .filter(|token| !token.is_empty())
        .collect()
}

pub(super) fn is_blank_query(query: &str) -> bool {
    query_tokens(query).is_empty()
}

pub(super) fn best_match(fields: &[SearchText<'_>], query: &str) -> Option<SearchScore> {
    let tokens = query_tokens(query);
    if tokens.is_empty() {
        return Some(SearchScore {
            field: SearchField::Label,
            score: 0,
        });
    }

    let mut best_by_field = None;
    for field in [SearchField::Label, SearchField::Secondary, SearchField::Breadcrumb] {
        let field_texts = fields
            .iter()
            .filter(|candidate| candidate.field == field)
            .map(|candidate| candidate.text)
            .collect::<Vec<_>>();
        if field_texts.is_empty() {
            continue;
        }

        let mut total = 0;
        for token in &tokens {
            let Some(token_score) = field_texts
                .iter()
                .filter_map(|text| fuzzy_score(text, token))
                .max()
            else {
                total = i32::MIN;
                break;
            };
            total += token_score;
        }

        if total != i32::MIN {
            best_by_field = Some(SearchScore {
                field,
                score: total,
            });
            break;
        }
    }

    best_by_field
}

pub(super) fn rank_matches<T>(mut matches: Vec<SearchMatch<T>>) -> Vec<T> {
    matches.sort_by(|left, right| {
        left.score
            .field
            .cmp(&right.score.field)
            .then_with(|| right.score.score.cmp(&left.score.score))
            .then_with(|| left.source_index.cmp(&right.source_index))
    });
    matches.into_iter().map(|matched| matched.item).collect()
}

fn fuzzy_score(text: &str, token: &str) -> Option<i32> {
    if token.is_empty() {
        return Some(0);
    }

    let text_lower = text.to_ascii_lowercase();
    if let Some(index) = text_lower.find(token) {
        let boundary_bonus = if is_boundary(&text_lower, index) { 30 } else { 0 };
        return Some(10_000 + boundary_bonus - index as i32);
    }

    let mut score = 0;
    let mut search_start = 0;
    let mut previous_match = None;
    for ch in token.chars() {
        let remainder = &text_lower[search_start..];
        let Some(relative_index) = remainder.find(ch) else {
            return None;
        };
        let absolute_index = search_start + relative_index;
        score += 100;
        if previous_match == Some(absolute_index.saturating_sub(1)) {
            score += 25;
        }
        if is_boundary(&text_lower, absolute_index) {
            score += 15;
        }
        score -= relative_index as i32;
        previous_match = Some(absolute_index);
        search_start = absolute_index + ch.len_utf8();
    }

    Some(score - text_lower.len() as i32)
}

fn is_boundary(text: &str, index: usize) -> bool {
    if index == 0 {
        return true;
    }
    let mut previous = text[..index].chars().rev();
    let Some(prev) = previous.next() else {
        return true;
    };
    !prev.is_ascii_alphanumeric()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ordered_subsequence_is_required() {
        assert!(fuzzy_score("Firefox", "fx").is_some());
        assert!(fuzzy_score("Firefox", "xf").is_none());
    }

    #[test]
    fn all_whitespace_tokens_must_match() {
        let fields = [
            SearchText {
                field: SearchField::Label,
                text: "Visual Studio Code",
            },
            SearchText {
                field: SearchField::Secondary,
                text: "editor",
            },
        ];

        assert!(best_match(&fields, "code editor").is_some());
        assert!(best_match(&fields, "code browser").is_none());
    }

    #[test]
    fn label_field_beats_secondary_field() {
        let label = best_match(
            &[SearchText {
                field: SearchField::Label,
                text: "Network Connections",
            }],
            "net",
        )
        .expect("label match");
        let secondary = best_match(
            &[SearchText {
                field: SearchField::Secondary,
                text: "network",
            }],
            "net",
        )
        .expect("secondary match");

        let ranked = rank_matches(vec![
            SearchMatch {
                item: "secondary",
                score: secondary,
                source_index: 0,
            },
            SearchMatch {
                item: "label",
                score: label,
                source_index: 1,
            },
        ]);

        assert_eq!(ranked, vec!["label", "secondary"]);
    }

    #[test]
    fn substring_beats_non_contiguous_fuzzy_match() {
        let substring = fuzzy_score("Terminal", "term").expect("substring");
        let fuzzy = fuzzy_score("Telemetry Renderer Module", "term").expect("fuzzy");

        assert!(substring > fuzzy);
    }

    #[test]
    fn source_order_breaks_equal_score_ties() {
        let score = SearchScore {
            field: SearchField::Label,
            score: 10,
        };

        let ranked = rank_matches(vec![
            SearchMatch {
                item: "second",
                score,
                source_index: 1,
            },
            SearchMatch {
                item: "first",
                score,
                source_index: 0,
            },
        ]);

        assert_eq!(ranked, vec!["first", "second"]);
    }
}
```

- [x] **Step 2: Register the module**

In `src/app.rs`, add the module beside `mod panel;`:

```rust
mod panel;
mod panel_search;
```

- [x] **Step 3: Run the matcher tests**

Run:

```bash
cargo test app::panel_search
```

Expected: all `app::panel_search` tests pass.

- [x] **Step 4: Commit**

```bash
git add src/app.rs src/app/panel_search.rs
git commit -m "feat: add panel menu fuzzy matcher"
```

---

### Task 2: Apply Fuzzy Ranking to Configured Panel Menus

**Files:**
- Modify: `src/app/panel.rs`

- [x] **Step 1: Add failing configured-menu search tests**

In the existing `#[cfg(test)] mod tests` in `src/app/panel.rs`, add:

```rust
#[test]
fn menu_search_ranks_label_matches_before_keyword_matches() {
    let mut menu = menu_with_apps_browser();
    menu.items.push(crate::config::MenuItemConfig {
        label: "Settings".into(),
        icon: None,
        keywords: vec!["browser".into()],
        action: Some(crate::config::PanelAction::Command(
            crate::config::CommandConfig::Shell("settings".into()),
        )),
        children: vec![],
    });

    let results = search_menu_entries(&menu, "browser");

    assert_eq!(results[0].label, "Apps / Browser");
    assert_eq!(results[1].label, "Settings");
}

#[test]
fn menu_search_uses_ordered_fuzzy_matching() {
    let menu = menu_with_apps_browser();

    let results = search_menu_entries(&menu, "brsr");

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].label, "Apps / Browser");
    assert!(search_menu_entries(&menu, "rb").is_empty());
}

#[test]
fn menu_search_matches_breadcrumb_as_lowest_priority() {
    let menu = crate::config::MenuConfig {
        name: "main".into(),
        show_icons: false,
        prompt: None,
        source: None,
        items: vec![
            crate::config::MenuItemConfig {
                label: "Apps".into(),
                icon: None,
                keywords: vec![],
                action: None,
                children: vec![crate::config::MenuItemConfig {
                    label: "Firefox".into(),
                    icon: None,
                    keywords: vec![],
                    action: Some(crate::config::PanelAction::Command(
                        crate::config::CommandConfig::Shell("firefox".into()),
                    )),
                    children: vec![],
                }],
            },
            crate::config::MenuItemConfig {
                label: "App Settings".into(),
                icon: None,
                keywords: vec![],
                action: Some(crate::config::PanelAction::Command(
                    crate::config::CommandConfig::Shell("settings".into()),
                )),
                children: vec![],
            },
        ],
    };

    let results = search_menu_entries(&menu, "app");

    assert_eq!(
        results
            .iter()
            .map(|entry| entry.label.as_str())
            .collect::<Vec<_>>(),
        vec!["App Settings", "Apps", "Apps / Firefox"]
    );
}

#[test]
fn menu_search_requires_every_query_token() {
    let menu = menu_with_apps_browser();

    assert_eq!(search_menu_entries(&menu, "apps browser").len(), 1);
    assert!(search_menu_entries(&menu, "apps terminal").is_empty());
}
```

- [x] **Step 2: Run tests to verify they fail**

Run:

```bash
cargo test app::panel::tests::menu_search_
```

Expected: at least `menu_search_uses_ordered_fuzzy_matching` and breadcrumb/ordering tests fail under substring search.

- [x] **Step 3: Replace configured menu search implementation**

In `src/app/panel.rs`, import the matcher near the top:

```rust
use super::panel_search::{self, SearchField, SearchMatch, SearchText};
```

Replace `search_menu_entries` and `collect_search_menu_entries` with:

```rust
pub(super) fn search_menu_entries(
    menu: &crate::config::MenuConfig,
    query: &str,
) -> Vec<MenuEntryView> {
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
    labels: &mut Vec<String>,
    path: &mut Vec<usize>,
    source_index: &mut usize,
    results: &mut Vec<SearchMatch<MenuEntryView>>,
) {
    for (index, item) in items.iter().enumerate() {
        labels.push(item.label.clone());
        path.push(index);
        let current_source_index = *source_index;
        *source_index += 1;

        let breadcrumb = labels
            .iter()
            .take(labels.len().saturating_sub(1))
            .map(String::as_str)
            .collect::<Vec<_>>()
            .join(" / ");
        let keyword_text = item.keywords.join(" ");
        let fields = [
            SearchText {
                field: SearchField::Label,
                text: item.label.as_str(),
            },
            SearchText {
                field: SearchField::Secondary,
                text: keyword_text.as_str(),
            },
            SearchText {
                field: SearchField::Breadcrumb,
                text: breadcrumb.as_str(),
            },
        ];

        if let Some(score) = panel_search::best_match(&fields, query) {
            results.push(SearchMatch {
                item: MenuEntryView {
                    label: labels.join(" / "),
                    icon: item.icon.clone(),
                    path: path.clone(),
                    target: MenuEntryTarget::StaticPath(path.clone()),
                    is_submenu: !item.children.is_empty(),
                },
                score,
                source_index: current_source_index,
            });
        }

        collect_search_menu_entries(
            &item.children,
            query,
            labels,
            path,
            source_index,
            results,
        );
        path.pop();
        labels.pop();
    }
}
```

- [x] **Step 4: Run configured menu tests**

Run:

```bash
cargo test app::panel::tests::menu_search_
```

Expected: configured menu search tests pass.

- [x] **Step 5: Commit**

```bash
git add src/app/panel.rs
git commit -m "feat: rank configured panel menu search"
```

---

### Task 3: Apply Fuzzy Ranking to Installed Apps

**Files:**
- Modify: `src/app/panel.rs`

- [x] **Step 1: Add failing installed-app search tests**

In the existing `#[cfg(test)] mod tests` in `src/app/panel.rs`, add:

```rust
#[test]
fn installed_apps_search_ranks_visible_label_before_metadata() {
    let mut settings = installed_app("Settings", "settings.desktop");
    settings.keywords = vec!["network".into()];
    let mut network = installed_app("Network Connections", "network.desktop");
    network.comment = Some("Configure links".into());
    let mut app = app_with_installed_apps_menu(vec![settings, network]);
    open_apps_menu_panel(&mut app);

    app.panel.search_query = "net".into();

    let entries = app.active_menu_entries();
    assert_eq!(
        entries
            .iter()
            .map(|entry| entry.label.as_str())
            .collect::<Vec<_>>(),
        vec!["Network Connections", "Settings"]
    );
}

#[test]
fn installed_apps_search_matches_tokens_across_fields() {
    let mut code = installed_app("Visual Studio Code", "code.desktop");
    code.keywords = vec!["editor".into()];
    let mut app = app_with_installed_apps_menu(vec![code]);
    open_apps_menu_panel(&mut app);

    app.panel.search_query = "code editor".into();

    let entries = app.active_menu_entries();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].label, "Visual Studio Code");
}

#[test]
fn installed_apps_search_preserves_source_order_for_blank_query() {
    let mut app = app_with_installed_apps_menu(vec![
        installed_app("Zed", "zed.desktop"),
        installed_app("Alpha", "alpha.desktop"),
    ]);
    open_apps_menu_panel(&mut app);

    app.panel.search_query = "   ".into();

    let entries = app.active_menu_entries();
    assert_eq!(
        entries
            .iter()
            .map(|entry| entry.label.as_str())
            .collect::<Vec<_>>(),
        vec!["Zed", "Alpha"]
    );
}
```

- [x] **Step 2: Run tests to verify they fail**

Run:

```bash
cargo test app::panel::tests::installed_apps_search_
```

Expected: `installed_apps_search_ranks_visible_label_before_metadata` fails under current source-order substring filtering.

- [x] **Step 3: Replace installed app filtering with ranked matching**

Replace `installed_app_entries` and remove `installed_app_matches`:

```rust
fn installed_app_entries(
    apps: &[crate::desktop_apps::DesktopApplication],
    query: &str,
) -> Vec<MenuEntryView> {
    let mut counts = std::collections::HashMap::<String, usize>::new();
    for app in apps {
        *counts.entry(app.name.to_ascii_lowercase()).or_default() += 1;
    }

    let mut entries = Vec::new();
    for (index, app) in apps.iter().enumerate() {
        let label = if counts
            .get(&app.name.to_ascii_lowercase())
            .copied()
            .unwrap_or(0)
            > 1
        {
            app.generic_name
                .as_ref()
                .filter(|value| !value.is_empty())
                .map(|generic| format!("{} ({generic})", app.name))
                .unwrap_or_else(|| format!("{} ({})", app.name, app.id))
        } else {
            app.name.clone()
        };
        let icon = app
            .icon
            .as_ref()
            .filter(|icon| crate::icon_theme::is_valid_icon_name(icon))
            .cloned();
        let view = MenuEntryView {
            label,
            icon,
            path: vec![index],
            target: MenuEntryTarget::InstalledApp(app.id.clone()),
            is_submenu: false,
        };

        if panel_search::is_blank_query(query) {
            entries.push(SearchMatch {
                item: view,
                score: panel_search::best_match(
                    &[SearchText {
                        field: SearchField::Label,
                        text: app.name.as_str(),
                    }],
                    "",
                )
                .expect("blank query matches"),
                source_index: index,
            });
            continue;
        }

        let generic_name = app.generic_name.as_deref().unwrap_or("");
        let comment = app.comment.as_deref().unwrap_or("");
        let keywords = app.keywords.join(" ");
        let fields = [
            SearchText {
                field: SearchField::Label,
                text: app.name.as_str(),
            },
            SearchText {
                field: SearchField::Secondary,
                text: generic_name,
            },
            SearchText {
                field: SearchField::Secondary,
                text: comment,
            },
            SearchText {
                field: SearchField::Secondary,
                text: keywords.as_str(),
            },
        ];

        if let Some(score) = panel_search::best_match(&fields, query) {
            entries.push(SearchMatch {
                item: view,
                score,
                source_index: index,
            });
        }
    }

    if panel_search::is_blank_query(query) {
        return entries.into_iter().map(|matched| matched.item).collect();
    }
    panel_search::rank_matches(entries)
}
```

- [x] **Step 4: Run installed-app tests**

Run:

```bash
cargo test app::panel::tests::installed_apps_
```

Expected: installed-app tests pass, including the existing hover cache test.

- [x] **Step 5: Commit**

```bash
git add src/app/panel.rs
git commit -m "feat: rank installed app panel menu search"
```

---

### Task 4: Apply Fuzzy Ranking to Dmenu Panel Menu Sources

**Files:**
- Modify: `src/app/panel.rs`

- [x] **Step 1: Add and update dmenu tests**

Update `dmenu_rich_entries_use_label_icon_keywords_and_value` so custom typed value comes after the fuzzy match:

```rust
assert_eq!(entries.len(), 2);
assert_eq!(entries[0].label, "Firefox");
assert_eq!(entries[0].icon.as_deref(), Some("firefox"));
assert_eq!(
    entries[0].target,
    MenuEntryTarget::DmenuValue("org.mozilla.firefox".into())
);
assert_eq!(entries[1].label, "browser");
assert!(matches!(
    entries[1].target,
    MenuEntryTarget::DmenuCustomValue(_)
));
```

Add:

```rust
#[test]
fn dmenu_search_uses_fuzzy_ranking() {
    let request = crate::ipc::protocol::DmenuRequest {
        prompt: None,
        entries: vec![
            crate::ipc::protocol::DmenuEntry::plain("Telemetry Renderer Module"),
            crate::ipc::protocol::DmenuEntry::plain("Terminal"),
        ],
        allow_custom: false,
    };

    let entries = dmenu_entry_views(&request, "term");

    assert_eq!(
        entries
            .iter()
            .map(|entry| entry.label.as_str())
            .collect::<Vec<_>>(),
        vec!["Terminal", "Telemetry Renderer Module"]
    );
}

#[test]
fn dmenu_custom_value_appears_after_fuzzy_matches() {
    let request = crate::ipc::protocol::DmenuRequest {
        prompt: None,
        entries: vec![crate::ipc::protocol::DmenuEntry::plain("Firefox")],
        allow_custom: true,
    };

    let entries = dmenu_entry_views(&request, "firfox");

    assert_eq!(entries[0].label, "Firefox");
    assert_eq!(entries[1].label, "firfox");
    assert!(matches!(
        entries[1].target,
        MenuEntryTarget::DmenuCustomValue(_)
    ));
}

#[test]
fn dmenu_blank_query_preserves_duplicate_source_order() {
    let request = crate::ipc::protocol::DmenuRequest {
        prompt: None,
        entries: vec![
            crate::ipc::protocol::DmenuEntry::plain("firefox"),
            crate::ipc::protocol::DmenuEntry::plain("kitty"),
            crate::ipc::protocol::DmenuEntry::plain("firefox"),
        ],
        allow_custom: true,
    };

    let entries = dmenu_entry_views(&request, " ");

    assert_eq!(
        entries
            .iter()
            .map(|entry| entry.label.as_str())
            .collect::<Vec<_>>(),
        vec!["firefox", "kitty", "firefox"]
    );
}
```

- [x] **Step 2: Run tests to verify they fail**

Run:

```bash
cargo test app::panel::tests::dmenu_
```

Expected: custom-row ordering and fuzzy-ranking tests fail before implementation.

- [x] **Step 3: Replace dmenu search implementation**

Replace `dmenu_entry_views` and remove `dmenu_entry_matches`:

```rust
pub(super) fn dmenu_entry_views(
    request: &crate::ipc::protocol::DmenuRequest,
    query: &str,
) -> Vec<MenuEntryView> {
    let exact_match = request
        .entries
        .iter()
        .any(|entry| entry.display_label() == query || entry.selected_value() == query);

    let mut matches = Vec::new();
    for (index, entry) in request.entries.iter().enumerate() {
        let view = MenuEntryView {
            label: entry.display_label().to_string(),
            icon: entry.icon.clone(),
            path: vec![index],
            target: MenuEntryTarget::DmenuValue(entry.selected_value().to_string()),
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

    if request.allow_custom && !query.is_empty() && !exact_match {
        entries.push(MenuEntryView {
            label: query.to_string(),
            icon: None,
            path: Vec::new(),
            target: MenuEntryTarget::DmenuCustomValue(query.to_string()),
            is_submenu: false,
        });
    }

    entries
}
```

- [x] **Step 4: Preserve rich dmenu opaque-value behavior**

Run:

```bash
cargo test app::panel::tests::dmenu_rich_search_does_not_match_opaque_value_when_label_exists app::panel::tests::dmenu_value_only_rich_entry_matches_and_displays_value
```

Expected: both pass. If `dmenu_value_only_rich_entry_matches_and_displays_value` fails, confirm `DmenuEntry::display_label()` returns the value for value-only rich entries and keep matching only `display_label()` plus keywords.

- [x] **Step 5: Run all dmenu tests**

Run:

```bash
cargo test app::panel::tests::dmenu_
```

Expected: all dmenu panel tests pass.

- [x] **Step 6: Commit**

```bash
git add src/app/panel.rs
git commit -m "feat: rank dmenu panel menu search"
```

---

### Task 5: Full Regression Pass and Cleanup

**Files:**
- Modify only if tests expose issues: `src/app/panel_search.rs`, `src/app/panel.rs`

- [x] **Step 1: Run the focused panel test suite**

Run:

```bash
cargo test app::panel
```

Expected: all `app::panel` tests pass.

- [x] **Step 2: Run all app search matcher tests**

Run:

```bash
cargo test app::panel_search app::panel::tests::menu_search_ app::panel::tests::installed_apps_search_ app::panel::tests::dmenu_
```

Expected: all targeted search tests pass.

- [x] **Step 3: Run formatting**

Run:

```bash
cargo fmt
```

Expected: command exits successfully with no output or only normal rustfmt completion.

- [x] **Step 4: Run full test suite**

Run:

```bash
cargo test
```

Expected: full test suite passes.

- [x] **Step 5: Inspect final diff**

Run:

```bash
git diff -- src/app.rs src/app/panel.rs src/app/panel_search.rs CONTEXT.md
```

Expected:
- `src/app/panel_search.rs` contains pure matcher logic and tests.
- `src/app/panel.rs` no longer has substring-only search helpers.
- `CONTEXT.md` remains the previously agreed domain documentation.
- No `Cargo.toml` dependency changes.

- [x] **Step 6: Commit cleanup**

```bash
git add src/app.rs src/app/panel.rs src/app/panel_search.rs
git commit -m "test: cover fuzzy panel menu search behavior"
```

---

## Self-Review

- Spec coverage: configured Panel Menus, installed apps, dmenu sources, default behavior, ordered subsequence, ASCII case-insensitivity, token matching, ranked results, source-order tie-breaking, empty-query order, substring preference, field priority, breadcrumb priority, all matches, in-repo matcher, and dmenu custom-row semantics are each covered by tasks.
- Placeholder scan: no `TBD`, `TODO`, or unspecified test steps remain.
- Type consistency: `SearchField`, `SearchText`, `SearchScore`, and `SearchMatch` are defined in Task 1 and reused consistently in later tasks.
