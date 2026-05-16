# Installed Apps Panel Menu Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add an internal Panel Menu source that lists installed desktop applications and launches the selected application.

**Architecture:** Extend existing named `MenuConfig` with a mutually exclusive `MenuSourceConfig`. Static menu items keep the current path; dynamic installed-app menus resolve entries when the panel opens, store a stable snapshot while open, and activate source-owned launch behavior through the existing `src/command.rs` process-launch boundary.

**Tech Stack:** Rust 2024, KDL config parsing, Iced/Iced Layershell panel UI, freedesktop desktop-entry metadata, Tokio tests where process launching is involved.

---

## File Structure

- `Cargo.toml`: add a desktop-entry parsing dependency if one fits the freedesktop semantics; otherwise no dependency and implement the limited parser in `src/desktop_apps.rs`.
- `src/config/module.rs`: add `MenuSourceConfig` and `MenuSourceKind`; add `source: Option<MenuSourceConfig>` to `MenuConfig`.
- `src/config/parse/mod.rs`: parse `source "installed-apps"` inside `menus { menu ... }`; reject unknown source options and mixed `source` plus `item`.
- `src/config/parse/tests.rs`: parser tests for source syntax and validation errors.
- `src/config/tests.rs`: validation/default construction updates for `MenuConfig`.
- `src/config/validate.rs`: keep existing menu reference validation; skip static item traversal for source menus.
- `src/desktop_apps.rs`: discover installed GUI desktop applications, filter them, sort them, expose launch specs.
- `src/command.rs`: add a launch helper for desktop application argv derived from desktop-entry `Exec`; keep all spawning centralized here.
- `src/app/panel.rs`: resolve dynamic menu entries when a menu panel opens, search them, activate entries, and preserve snapshots while open.
- `src/app/render.rs`: pass dynamic menu entries through the existing `PanelRenderModel`.
- `src/surface/panel.rs`: add any model types needed for dynamic menu entry snapshots if they are UI-facing.
- `src/lib.rs` or `src/main.rs`: expose `desktop_apps` module according to the repo’s module layout.
- `CONTEXT.md`: already updated during grilling; only touch again if implementation reveals a domain correction.

## Task 1: Config Model And Parser

**Files:**
- Modify: `src/config/module.rs`
- Modify: `src/config/parse/mod.rs`
- Modify: `src/config/parse/tests.rs`
- Modify: `src/config/tests.rs`

- [ ] **Step 1: Add failing parser tests for a dynamic menu source**

Add these tests near the existing menu parser tests in `src/config/parse/tests.rs`:

```rust
#[test]
fn parse_menu_with_installed_apps_source() {
    let kdl = r#"
menus {
    menu "apps" {
        source "installed-apps"
    }
}
"#;
    let config = parse(kdl).unwrap();

    assert_eq!(config.menus.len(), 1);
    assert_eq!(config.menus[0].name, "apps");
    assert_eq!(
        config.menus[0].source,
        Some(crate::config::MenuSourceConfig {
            kind: crate::config::MenuSourceKind::InstalledApps,
        })
    );
    assert!(config.menus[0].items.is_empty());
}

#[test]
fn parse_menu_rejects_source_and_items() {
    let kdl = r#"
menus {
    menu "apps" {
        source "installed-apps"
        item "Terminal" {
            command "kitty"
        }
    }
}
"#;
    let err = parse(kdl).unwrap_err().to_string();
    assert!(
        err.contains("menu cannot mix source and item"),
        "got: {err}"
    );
}

#[test]
fn parse_menu_rejects_unknown_source() {
    let kdl = r#"
menus {
    menu "apps" {
        source "packages"
    }
}
"#;
    let err = parse(kdl).unwrap_err().to_string();
    assert!(err.contains("unknown menu source"), "got: {err}");
}

#[test]
fn parse_menu_rejects_source_options_for_now() {
    let kdl = r#"
menus {
    menu "apps" {
        source "installed-apps" show-icons=#true
    }
}
"#;
    let err = parse(kdl).unwrap_err().to_string();
    assert!(
        err.contains("source options are not supported"),
        "got: {err}"
    );
}
```

- [ ] **Step 2: Run parser tests and verify they fail**

Run:

```bash
cargo test config::parse::tests::parse_menu_with_installed_apps_source config::parse::tests::parse_menu_rejects_source_and_items config::parse::tests::parse_menu_rejects_unknown_source config::parse::tests::parse_menu_rejects_source_options_for_now
```

Expected: compile failure for missing `source`, `MenuSourceConfig`, and `MenuSourceKind`.

- [ ] **Step 3: Add menu source config types**

In `src/config/module.rs`, add:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MenuSourceKind {
    InstalledApps,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MenuSourceConfig {
    pub kind: MenuSourceKind,
}
```

Change `MenuConfig` to:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MenuConfig {
    pub name: String,
    pub source: Option<MenuSourceConfig>,
    pub items: Vec<MenuItemConfig>,
}
```

- [ ] **Step 4: Update test constructors to include `source: None`**

Search:

```bash
rg -n "MenuConfig \\{" src
```

For each existing static menu literal, add:

```rust
source: None,
```

For the dynamic-source parser test, use the `Some(MenuSourceConfig { ... })` from Step 1.

- [ ] **Step 5: Parse `source "installed-apps"`**

In `src/config/parse/mod.rs`, import the new config types:

```rust
use super::{
    BarConfig, CommandPolicy, Config, MenuConfig, MenuItemConfig, MenuSourceConfig,
    MenuSourceKind, ModuleConfig,
};
```

Replace the body of `parse_menus_section` menu child handling with logic equivalent to:

```rust
let mut source = None;
let mut item_nodes = Vec::new();
if let Some(menu_children) = node.children() {
    for child in menu_children.nodes() {
        match child.name().value() {
            "source" => {
                if source.is_some() {
                    return Err(menu_ctx.node_err(node, "menu can declare only one source", None));
                }
                source = Some(parse_menu_source(child, &menu_ctx.child("source"))?);
            }
            "item" => item_nodes.push(child),
            other => {
                return Err(menu_ctx.node_err(
                    child,
                    format!("expected node name \"item\" or \"source\", got {other:?}"),
                    None,
                ));
            }
        }
    }
}

if source.is_some() && !item_nodes.is_empty() {
    return Err(menu_ctx.node_err(node, "menu cannot mix source and item nodes", None));
}

let items = parse_menu_items_from_nodes(item_nodes, &menu_ctx, command_policy)?;
menus.push(MenuConfig { name, source, items });
```

Add these helpers below `parse_menus_section`:

```rust
fn parse_menu_source(node: &KdlNode, ctx: &ParseCtx) -> ConfigResult<MenuSourceConfig> {
    if node.entries().len() != 1 {
        return Err(ctx.node_err(node, "expected one menu source string", None));
    }
    if node.children().is_some() || node.entries().iter().any(|entry| entry.name().is_some()) {
        return Err(ctx.node_err(node, "source options are not supported yet", None));
    }
    let Some(kind) = node.entries()[0].value().as_string() else {
        return Err(ctx.entry_err(&node.entries()[0], "expected menu source string", None));
    };
    match kind {
        "installed-apps" => Ok(MenuSourceConfig {
            kind: MenuSourceKind::InstalledApps,
        }),
        other => Err(ctx.entry_err(
            &node.entries()[0],
            format!("unknown menu source {other:?}"),
            Some("expected `installed-apps`".into()),
        )),
    }
}

fn parse_menu_items_from_nodes(
    nodes: Vec<&KdlNode>,
    ctx: &ParseCtx,
    command_policy: &CommandPolicy,
) -> ConfigResult<Vec<MenuItemConfig>> {
    let mut items = Vec::new();
    for (item_idx, node) in nodes.into_iter().enumerate() {
        items.push(parse_menu_item(
            node,
            &ctx.child("item").index(item_idx),
            command_policy,
        )?);
    }
    Ok(items)
}
```

Keep `parse_menu_items` for nested static item parsing, or rewrite it to call `parse_menu_items_from_nodes(children.nodes().iter().collect(), ctx, command_policy)`.

- [ ] **Step 6: Run parser tests**

Run:

```bash
cargo test config::parse::tests::parse_menu
```

Expected: all menu parser tests pass.

- [ ] **Step 7: Commit**

```bash
rtk git add src/config/module.rs src/config/parse/mod.rs src/config/parse/tests.rs src/config/tests.rs
rtk git commit -m "feat: parse installed apps menu source"
```

## Task 2: Desktop Application Discovery

**Files:**
- Create: `src/desktop_apps.rs`
- Modify: `src/lib.rs` or `src/main.rs`
- Modify: `Cargo.toml` if using a parser crate

- [ ] **Step 1: Pick parser dependency or local parser**

Check crates for desktop-entry parsing. Prefer a crate that exposes name, generic name, comment, keywords, icon, hidden/no-display, terminal flag, OnlyShowIn/NotShowIn, desktop ID, and Exec. If no crate covers those, implement a small local parser in `src/desktop_apps.rs`.

If adding a crate, update `Cargo.toml` with the selected dependency and commit the lockfile change if the repo has `Cargo.lock`.

- [ ] **Step 2: Add failing unit tests for filtering and sorting**

Create `src/desktop_apps.rs` with tests first:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DesktopApplication {
    pub id: String,
    pub name: String,
    pub generic_name: Option<String>,
    pub comment: Option<String>,
    pub keywords: Vec<String>,
    pub icon: Option<String>,
    pub exec: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(id: &str, body: &str) -> RawDesktopEntry {
        RawDesktopEntry {
            id: id.into(),
            body: body.into(),
        }
    }

    #[test]
    fn filters_to_launchable_gui_apps_and_sorts_by_name() {
        let entries = vec![
            entry("z.desktop", "[Desktop Entry]\nType=Application\nName=Zed\nExec=zed\n"),
            entry("hidden.desktop", "[Desktop Entry]\nType=Application\nName=Hidden\nExec=hidden\nHidden=true\n"),
            entry("terminal.desktop", "[Desktop Entry]\nType=Application\nName=Terminal App\nExec=vim\nTerminal=true\n"),
            entry("a.desktop", "[Desktop Entry]\nType=Application\nName=Alpha\nExec=alpha\nNoDisplay=false\n"),
            entry("link.desktop", "[Desktop Entry]\nType=Link\nName=Docs\nURL=https://example.com\n"),
        ];

        let apps = applications_from_entries(entries, &DesktopEnvironment::unknown());

        assert_eq!(
            apps.iter().map(|app| app.name.as_str()).collect::<Vec<_>>(),
            vec!["Alpha", "Zed"]
        );
    }

    #[test]
    fn excludes_only_show_in_when_current_desktop_is_unknown() {
        let entries = vec![entry(
            "gnome-only.desktop",
            "[Desktop Entry]\nType=Application\nName=GNOME Only\nExec=gnome-app\nOnlyShowIn=GNOME;\n",
        )];

        let apps = applications_from_entries(entries, &DesktopEnvironment::unknown());

        assert!(apps.is_empty());
    }

    #[test]
    fn honors_current_desktop_for_show_in_rules() {
        let env = DesktopEnvironment::from_current_desktop("GNOME:niri");
        let entries = vec![
            entry("gnome.desktop", "[Desktop Entry]\nType=Application\nName=GNOME App\nExec=app\nOnlyShowIn=GNOME;\n"),
            entry("blocked.desktop", "[Desktop Entry]\nType=Application\nName=Blocked\nExec=blocked\nNotShowIn=niri;\n"),
        ];

        let apps = applications_from_entries(entries, &env);

        assert_eq!(apps.len(), 1);
        assert_eq!(apps[0].name, "GNOME App");
    }

    #[test]
    fn extracts_search_metadata() {
        let entries = vec![entry(
            "browser.desktop",
            "[Desktop Entry]\nType=Application\nName=Browser\nGenericName=Web Browser\nComment=Browse the web\nKeywords=web;internet;\nIcon=browser\nExec=browser %U\n",
        )];

        let apps = applications_from_entries(entries, &DesktopEnvironment::unknown());

        assert_eq!(apps[0].generic_name.as_deref(), Some("Web Browser"));
        assert_eq!(apps[0].comment.as_deref(), Some("Browse the web"));
        assert_eq!(apps[0].keywords, vec!["web", "internet"]);
        assert_eq!(apps[0].icon.as_deref(), Some("browser"));
    }
}
```

The concrete parser implementation may rename `RawDesktopEntry`, but keep the public test intent unchanged.

- [ ] **Step 3: Run discovery tests and verify they fail**

Run:

```bash
cargo test desktop_apps
```

Expected: compile failure for missing discovery types/functions.

- [ ] **Step 4: Implement discovery model and filtering**

Implement in `src/desktop_apps.rs`:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DesktopApplication {
    pub id: String,
    pub name: String,
    pub generic_name: Option<String>,
    pub comment: Option<String>,
    pub keywords: Vec<String>,
    pub icon: Option<String>,
    pub exec: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawDesktopEntry {
    pub id: String,
    pub body: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DesktopEnvironment {
    names: Vec<String>,
    known: bool,
}

impl DesktopEnvironment {
    pub fn unknown() -> Self {
        Self { names: Vec::new(), known: false }
    }

    pub fn current() -> Self {
        std::env::var("XDG_CURRENT_DESKTOP")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .map(|value| Self::from_current_desktop(&value))
            .unwrap_or_else(Self::unknown)
    }

    pub fn from_current_desktop(value: &str) -> Self {
        Self {
            names: value
                .split(':')
                .map(|name| name.trim().to_ascii_lowercase())
                .filter(|name| !name.is_empty())
                .collect(),
            known: true,
        }
    }

    fn contains(&self, name: &str) -> bool {
        self.names.iter().any(|candidate| candidate == &name.to_ascii_lowercase())
    }
}
```

Implement `applications_from_entries(entries, env)` so it:

- accepts only `Type=Application`
- requires non-empty `Name` and `Exec`
- excludes `Hidden=true`
- excludes `NoDisplay=true`
- excludes `Terminal=true`
- excludes `OnlyShowIn` when env is unknown
- includes `OnlyShowIn` only when env contains one listed value
- excludes when `NotShowIn` contains the current env
- sorts by lowercase `name`, then lowercase `id`

- [ ] **Step 5: Add filesystem discovery**

Add:

```rust
pub fn discover_installed_applications() -> std::io::Result<Vec<DesktopApplication>> {
    discover_installed_applications_in_data_dirs(xdg_data_dirs(), &DesktopEnvironment::current())
}
```

Add testable helpers:

```rust
fn xdg_data_dirs() -> Vec<std::path::PathBuf> {
    let mut dirs = Vec::new();
    let data_home = std::env::var_os("XDG_DATA_HOME")
        .map(std::path::PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| std::path::PathBuf::from(home).join(".local/share")));
    if let Some(data_home) = data_home {
        dirs.push(data_home);
    }
    match std::env::var_os("XDG_DATA_DIRS") {
        Some(value) => dirs.extend(std::env::split_paths(&value)),
        None => {
            dirs.push("/usr/local/share".into());
            dirs.push("/usr/share".into());
        }
    }
    dirs
}
```

Read `.desktop` files under each `applications/` subdirectory. Derive desktop IDs from the relative path below `applications/`, replacing `/` with `-`, matching freedesktop desktop ID convention.

- [ ] **Step 6: Expose the module**

In the repo root module file, add:

```rust
pub mod desktop_apps;
```

If this is a binary-only crate without `src/lib.rs`, add:

```rust
mod desktop_apps;
```

to `src/main.rs` or the existing central module declaration file.

- [ ] **Step 7: Run discovery tests**

Run:

```bash
cargo test desktop_apps
```

Expected: all desktop app discovery tests pass.

- [ ] **Step 8: Commit**

```bash
rtk git add Cargo.toml Cargo.lock src/desktop_apps.rs src/lib.rs src/main.rs
rtk git commit -m "feat: discover installed desktop applications"
```

Only add files that exist or changed.

## Task 3: Desktop Exec Launch Helper

**Files:**
- Modify: `src/command.rs`
- Modify: `src/desktop_apps.rs`

- [ ] **Step 1: Add failing tests for desktop Exec argv conversion**

In `src/command.rs` tests, add:

```rust
#[test]
fn desktop_exec_removes_file_field_codes_for_plain_launch() {
    let argv = desktop_exec_to_argv("firefox %U --new-window").unwrap();
    assert_eq!(argv.program, "firefox");
    assert_eq!(argv.args, vec!["--new-window"]);
}

#[test]
fn desktop_exec_unescapes_literal_percent() {
    let argv = desktop_exec_to_argv("app --label 100%%").unwrap();
    assert_eq!(argv.program, "app");
    assert_eq!(argv.args, vec!["--label", "100%"]);
}

#[test]
fn desktop_exec_removes_metadata_field_codes() {
    let argv = desktop_exec_to_argv("app %i %c %k").unwrap();
    assert_eq!(argv.program, "app");
    assert!(argv.args.is_empty());
}

#[test]
fn desktop_exec_preserves_quoted_arguments() {
    let argv = desktop_exec_to_argv("app --name \"Two Words\"").unwrap();
    assert_eq!(argv.program, "app");
    assert_eq!(argv.args, vec!["--name", "Two Words"]);
}
```

- [ ] **Step 2: Run command tests and verify they fail**

Run:

```bash
cargo test command::tests::desktop_exec
```

Expected: compile failure for missing `desktop_exec_to_argv`.

- [ ] **Step 3: Implement `DesktopLaunchArgv` and parser**

In `src/command.rs`, add:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DesktopLaunchArgv {
    pub program: String,
    pub args: Vec<String>,
}
```

Add:

```rust
pub fn desktop_exec_to_argv(exec: &str) -> std::io::Result<DesktopLaunchArgv> {
    let tokens = split_desktop_exec(exec)?;
    let mut cleaned = Vec::new();
    for token in tokens {
        let token = strip_desktop_field_codes(&token);
        if !token.is_empty() {
            cleaned.push(token);
        }
    }
    let Some(program) = cleaned.first().cloned() else {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "desktop Exec produced no program",
        ));
    };
    Ok(DesktopLaunchArgv {
        program,
        args: cleaned.into_iter().skip(1).collect(),
    })
}
```

Implement `split_desktop_exec` with shell-like quote handling for spaces, single quotes, double quotes, and backslash escapes. Do not run through `sh -c`.

Implement `strip_desktop_field_codes` so `%f`, `%F`, `%u`, `%U`, `%i`, `%c`, and `%k` are removed; `%%` becomes `%`; unknown field codes are removed rather than passed to the process.

- [ ] **Step 4: Add launch function behind the central boundary**

In `src/command.rs`, add:

```rust
pub fn spawn_desktop_application_fire_and_forget(
    app: &crate::desktop_apps::DesktopApplication,
) -> std::io::Result<SpawnResult> {
    let argv = desktop_exec_to_argv(&app.exec)?;
    let borrowed: Vec<&str> = argv.args.iter().map(String::as_str).collect();
    spawn_command_fire_and_forget(CommandSpec::Argv {
        program: &argv.program,
        args: &borrowed,
    })
}
```

- [ ] **Step 5: Run command tests**

Run:

```bash
cargo test command::tests::desktop_exec
```

Expected: all desktop Exec tests pass.

- [ ] **Step 6: Commit**

```bash
rtk git add src/command.rs src/desktop_apps.rs
rtk git commit -m "feat: launch desktop applications via command boundary"
```

## Task 4: Dynamic Panel Menu Snapshot

**Files:**
- Modify: `src/app/panel.rs`
- Modify: `src/surface/panel.rs`
- Modify: `src/app/render.rs`

- [ ] **Step 1: Add failing panel tests for dynamic entries**

In `src/app/panel.rs` tests, add:

```rust
fn installed_app(name: &str, id: &str) -> crate::desktop_apps::DesktopApplication {
    crate::desktop_apps::DesktopApplication {
        id: id.into(),
        name: name.into(),
        generic_name: None,
        comment: None,
        keywords: vec![],
        icon: None,
        exec: name.to_ascii_lowercase(),
    }
}

#[test]
fn installed_apps_menu_entries_use_open_snapshot() {
    let mut app = app_with_installed_apps_menu(vec![
        installed_app("Alpha", "alpha.desktop"),
        installed_app("Zed", "zed.desktop"),
    ]);

    open_apps_menu_panel(&mut app);

    let entries = app.active_menu_entries();
    assert_eq!(
        entries.iter().map(|entry| entry.label.as_str()).collect::<Vec<_>>(),
        vec!["Alpha", "Zed"]
    );
}

#[test]
fn installed_apps_search_matches_keywords_and_comment() {
    let mut browser = installed_app("Browser", "browser.desktop");
    browser.comment = Some("Browse the web".into());
    browser.keywords = vec!["internet".into()];
    let mut app = app_with_installed_apps_menu(vec![browser, installed_app("Editor", "editor.desktop")]);
    open_apps_menu_panel(&mut app);

    app.panel.search_query = "internet".into();

    let entries = app.active_menu_entries();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].label, "Browser");
}
```

Add local test helpers in the same test module:

```rust
fn app_with_installed_apps_menu(apps: Vec<crate::desktop_apps::DesktopApplication>) -> App {
    let mut config = crate::config::Config::default();
    config.menus = vec![crate::config::MenuConfig {
        name: "apps".into(),
        source: Some(crate::config::MenuSourceConfig {
            kind: crate::config::MenuSourceKind::InstalledApps,
        }),
        items: vec![],
    }];
    let (mut app, _) = App::boot(config, vec![]);
    app.panel.installed_apps_for_test = Some(apps);
    app
}

fn open_apps_menu_panel(app: &mut App) {
    let origin = iced::window::Id::unique();
    app.surface_mut(origin);
    let _ = app.update(Message::Panel(crate::app::messages::PanelEvent::Open {
        origin_window: origin,
        origin: crate::surface::panel::PanelOrigin::Bar,
        content: crate::surface::panel::PanelContent::Menu("apps".into()),
        anchor: None,
    }));
}
```

If test-only injection should not live on `PanelSystem`, create a small `DesktopAppProvider` trait instead. Keep the production path using `discover_installed_applications()`.

- [ ] **Step 2: Run panel tests and verify they fail**

Run:

```bash
cargo test app::panel::tests::installed_apps
```

Expected: compile failure for missing snapshot storage/test injection.

- [ ] **Step 3: Add source entry snapshot types**

In `src/app/panel.rs`, extend `MenuEntryView`:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum MenuEntryTarget {
    StaticPath(Vec<usize>),
    InstalledApp(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct MenuEntryView {
    pub(super) label: String,
    pub(super) path: Vec<usize>,
    pub(super) target: MenuEntryTarget,
    pub(super) is_submenu: bool,
}
```

Keep `path` during transition so existing static code and renderer do not need broad changes in the same step.

Extend `PanelSystem`:

```rust
pub(super) installed_apps_snapshot: Vec<crate::desktop_apps::DesktopApplication>,
#[cfg(test)]
pub(super) installed_apps_for_test: Option<Vec<crate::desktop_apps::DesktopApplication>>,
```

Update `clear()` to clear the snapshot.

- [ ] **Step 4: Resolve installed apps when opening a source menu**

In `App::open_panel`, before inserting `PanelMeta`, detect:

```rust
let is_installed_apps_menu = match &tree.content {
    crate::surface::panel::PanelContent::Menu(name) => self
        .config
        .menus
        .iter()
        .find(|menu| menu.name == *name)
        .and_then(|menu| menu.source.as_ref())
        .map(|source| matches!(source.kind, crate::config::MenuSourceKind::InstalledApps))
        .unwrap_or(false),
    _ => false,
};
if is_installed_apps_menu {
    self.panel.installed_apps_snapshot = self.load_installed_apps_for_panel();
}
```

Add:

```rust
fn load_installed_apps_for_panel(&self) -> Vec<crate::desktop_apps::DesktopApplication> {
    #[cfg(test)]
    if let Some(apps) = &self.panel.installed_apps_for_test {
        return apps.clone();
    }
    crate::desktop_apps::discover_installed_applications().unwrap_or_default()
}
```

- [ ] **Step 5: Generate active entries from source snapshot**

In `active_menu_tree_and_entries`, branch on `menu.source`:

```rust
let entries = match &menu.source {
    Some(source) if matches!(source.kind, crate::config::MenuSourceKind::InstalledApps) => {
        installed_app_entries(&self.panel.installed_apps_snapshot, &self.panel.search_query)
    }
    _ if self.panel.search_query.is_empty() => current_menu_entries(menu, &self.panel.menu_path),
    _ => search_menu_entries(menu, &self.panel.search_query),
};
```

Add:

```rust
fn installed_app_entries(
    apps: &[crate::desktop_apps::DesktopApplication],
    query: &str,
) -> Vec<MenuEntryView> {
    let query = query.to_ascii_lowercase();
    apps.iter()
        .filter(|app| query.is_empty() || installed_app_matches(app, &query))
        .enumerate()
        .map(|(index, app)| MenuEntryView {
            label: app.name.clone(),
            path: vec![index],
            target: MenuEntryTarget::InstalledApp(app.id.clone()),
            is_submenu: false,
        })
        .collect()
}

fn installed_app_matches(app: &crate::desktop_apps::DesktopApplication, query: &str) -> bool {
    app.name.to_ascii_lowercase().contains(query)
        || app.generic_name.as_deref().unwrap_or("").to_ascii_lowercase().contains(query)
        || app.comment.as_deref().unwrap_or("").to_ascii_lowercase().contains(query)
        || app.keywords.iter().any(|keyword| keyword.to_ascii_lowercase().contains(query))
}
```

Update `current_menu_entries` and `search_menu_entries` to set `target: MenuEntryTarget::StaticPath(item_path.clone())`.

- [ ] **Step 6: Run panel snapshot tests**

Run:

```bash
cargo test app::panel::tests::installed_apps
```

Expected: all installed apps panel tests pass.

- [ ] **Step 7: Commit**

```bash
rtk git add src/app/panel.rs src/app/render.rs src/surface/panel.rs
rtk git commit -m "feat: show installed apps in panel menus"
```

## Task 5: Dynamic Entry Activation

**Files:**
- Modify: `src/app/panel.rs`
- Modify: `src/command.rs`

- [ ] **Step 1: Add failing activation tests**

In `src/app/panel.rs` tests, add:

```rust
#[test]
fn activating_installed_app_closes_panel() {
    let mut app = app_with_installed_apps_menu(vec![installed_app("Alpha", "alpha.desktop")]);
    open_apps_menu_panel(&mut app);
    assert_eq!(app.surfaces.panel_popup_ids().count(), 1);

    let _ = app.update(Message::Panel(crate::app::messages::PanelEvent::ActivateIndex(0)));

    assert_eq!(app.surfaces.panel_popup_ids().count(), 0);
}
```

If the direct spawn makes this test launch a real process, add a `#[cfg(test)]` command recorder in `App` or `PanelSystem` and assert the recorded desktop ID instead:

```rust
assert_eq!(app.panel.launched_installed_app_ids_for_test, vec!["alpha.desktop"]);
```

- [ ] **Step 2: Run activation test and verify it fails**

Run:

```bash
cargo test app::panel::tests::activating_installed_app_closes_panel
```

Expected: failure because activation still assumes static `MenuItemConfig`.

- [ ] **Step 3: Branch activation by entry target**

In `activate_panel_entry`, after resolving `entry`, match:

```rust
match entry.target {
    MenuEntryTarget::InstalledApp(app_id) => {
        let Some(app) = self
            .panel
            .installed_apps_snapshot
            .iter()
            .find(|app| app.id == app_id)
            .cloned()
        else {
            return Task::none();
        };
        #[cfg(test)]
        {
            self.panel.launched_installed_app_ids_for_test.push(app.id.clone());
            return Task::batch(self.close_all_panels());
        }
        #[cfg(not(test))]
        {
            let _ = crate::command::spawn_desktop_application_fire_and_forget(&app);
            return Task::batch(self.close_all_panels());
        }
    }
    MenuEntryTarget::StaticPath(path) => {
        // Existing static menu activation logic uses `path`.
    }
}
```

Move the existing static item lookup/action dispatch into the `StaticPath` branch.

- [ ] **Step 4: Run activation tests**

Run:

```bash
cargo test app::panel::tests::activating_installed_app_closes_panel
cargo test app::panel::tests::activate
```

Expected: activation tests pass.

- [ ] **Step 5: Commit**

```bash
rtk git add src/app/panel.rs src/command.rs
rtk git commit -m "feat: launch installed apps from panel menu"
```

## Task 6: UI Metadata And Duplicate Labels

**Files:**
- Modify: `src/app/panel.rs`
- Modify: panel rendering file that displays `MenuEntryView` rows, likely `src/surface/panel.rs` or `src/app/render.rs`

- [ ] **Step 1: Add failing duplicate label test**

In `src/app/panel.rs` tests, add:

```rust
#[test]
fn duplicate_installed_app_names_are_disambiguated() {
    let mut app = app_with_installed_apps_menu(vec![
        installed_app("Terminal", "org.gnome.Terminal.desktop"),
        installed_app("Terminal", "com.raggesilver.BlackBox.desktop"),
    ]);
    open_apps_menu_panel(&mut app);

    let entries = app.active_menu_entries();

    assert_eq!(entries.len(), 2);
    assert!(entries[0].label.contains("Terminal"));
    assert!(entries[1].label.contains("Terminal"));
    assert_ne!(entries[0].label, entries[1].label);
}
```

- [ ] **Step 2: Run duplicate test and verify it fails**

Run:

```bash
cargo test app::panel::tests::duplicate_installed_app_names_are_disambiguated
```

Expected: failure because both labels are identical.

- [ ] **Step 3: Disambiguate duplicate display names**

In `installed_app_entries`, precompute names that occur more than once:

```rust
let mut counts = std::collections::HashMap::<String, usize>::new();
for app in apps {
    *counts.entry(app.name.to_ascii_lowercase()).or_default() += 1;
}
```

Set label:

```rust
let label = if counts.get(&app.name.to_ascii_lowercase()).copied().unwrap_or(0) > 1 {
    app.generic_name
        .as_ref()
        .filter(|value| !value.is_empty())
        .map(|generic| format!("{} ({generic})", app.name))
        .unwrap_or_else(|| format!("{} ({})", app.name, app.id))
} else {
    app.name.clone()
};
```

- [ ] **Step 4: Preserve best-effort icon metadata**

If the existing Panel Menu row model can show icons, add `icon: Option<String>` to `MenuEntryView` and populate it from `DesktopApplication.icon`. If the current renderer is text-only, leave icon data in the snapshot and do not block this feature on icon rendering.

- [ ] **Step 5: Run panel entry tests**

Run:

```bash
cargo test app::panel::tests::installed_apps app::panel::tests::duplicate_installed_app_names_are_disambiguated
```

Expected: all installed app entry tests pass.

- [ ] **Step 6: Commit**

```bash
rtk git add src/app/panel.rs src/app/render.rs src/surface/panel.rs
rtk git commit -m "feat: disambiguate installed app menu entries"
```

## Task 7: Validation And Regression Coverage

**Files:**
- Modify: `src/config/validate.rs`
- Modify: `src/config/tests.rs`
- Modify: `src/app/panel.rs`

- [ ] **Step 1: Add validation tests for source menus referenced by actions**

In `src/config/tests.rs`, add:

```rust
#[test]
fn validate_open_menu_accepts_source_menu() {
    let mut config = Config::default();
    config.menus = vec![MenuConfig {
        name: "apps".into(),
        source: Some(MenuSourceConfig {
            kind: MenuSourceKind::InstalledApps,
        }),
        items: vec![],
    }];
    config.bars[0].actions.bindings.push(ActionBinding {
        trigger: ActionTrigger::Click,
        action: PanelAction::OpenMenu("apps".into()),
    });

    validate(&config).unwrap();
}
```

Import the new source types at the top of the test module if needed.

- [ ] **Step 2: Run validation tests**

Run:

```bash
cargo test config::tests::validate_open_menu
```

Expected: tests pass. If this fails, update validation to treat sourced menus as declared menus while skipping recursive item validation when `items` is empty.

- [ ] **Step 3: Add empty source panel test**

In `src/app/panel.rs` tests, add:

```rust
#[test]
fn installed_apps_menu_can_be_empty() {
    let mut app = app_with_installed_apps_menu(vec![]);
    open_apps_menu_panel(&mut app);

    assert_eq!(app.surfaces.panel_popup_ids().count(), 1);
    assert!(app.active_menu_entries().is_empty());
}
```

- [ ] **Step 4: Run empty source test**

Run:

```bash
cargo test app::panel::tests::installed_apps_menu_can_be_empty
```

Expected: pass.

- [ ] **Step 5: Commit**

```bash
rtk git add src/config/validate.rs src/config/tests.rs src/app/panel.rs
rtk git commit -m "test: cover installed apps menu validation"
```

## Task 8: End-To-End Verification

**Files:**
- Modify only if verification finds defects.

- [ ] **Step 1: Run targeted test suite**

Run:

```bash
cargo test config::parse::tests::parse_menu config::tests::validate_open_menu app::panel::tests::installed_apps desktop_apps command::tests::desktop_exec
```

Expected: all targeted tests pass.

- [ ] **Step 2: Run full test suite**

Run:

```bash
cargo test
```

Expected: all tests pass.

- [ ] **Step 3: Run formatting**

Run:

```bash
cargo fmt --check
```

Expected: no formatting diff. If it fails, run `cargo fmt`, inspect the diff, and rerun `cargo fmt --check`.

- [ ] **Step 4: Manual smoke config**

Use a local test config containing:

```kdl
menus {
    menu "apps" {
        source "installed-apps"
    }
}
```

Bind an existing bar or module action to:

```kdl
open-menu "apps"
```

Run Niribar, open the menu, search for an installed GUI app, launch it, and confirm the Panel closes.

- [ ] **Step 5: Final commit**

If verification required fixes:

```bash
rtk git add Cargo.toml Cargo.lock src docs
rtk git commit -m "fix: complete installed apps panel menu verification"
```

If no fixes were needed, do not create an empty commit.

## Self-Review

- Spec coverage: dynamic Panel Menu source, GUI app filtering, search, refresh-on-open snapshot, activation/close behavior, desktop ID identity, duplicate names, best-effort icons, no new public PanelAction, and centralized process launch are covered.
- Placeholder scan: no `TBD`, `TODO`, or unspecified “add tests” steps remain; each task has concrete tests and commands.
- Type consistency: plan consistently uses `MenuSourceConfig`, `MenuSourceKind::InstalledApps`, `DesktopApplication`, `DesktopEnvironment`, `MenuEntryTarget`, and `DesktopLaunchArgv`.
