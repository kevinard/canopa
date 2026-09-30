# Add Panel Menu Entry Icons

Status: ready-for-human

## Goal

Add optional icons to Panel Menu entries, covering both explicit static menu items and dynamic installed-application menus.

## Background

Panel Menus currently render text-only entries. Installed desktop applications already carry optional `Icon=` metadata in `DesktopApplication.icon`, but the menu model and renderer do not expose or render it. Static menu entries also need first-class icon metadata so icons are not limited to installed applications.

Resolved domain language is captured in `CONTEXT.md` under **Panel Menu Entry Icon**.

## Decisions

- A **Panel Menu Entry Icon** is an optional XDG icon-theme name attached to a Panel Menu entry.
- Static menu items declare icons with a child node:

  ```kdl
  item "Terminal" {
      icon "utilities-terminal"
      command "kitty"
  }
  ```

- Dynamic installed-app entries use the `.desktop` `Icon=` field as their icon metadata.
- Icon rendering is enabled per menu with menu-header metadata:

  ```kdl
  menu "main" show-icons=#true {
      item "Terminal" {
          icon "utilities-terminal"
          command "kitty"
      }
  }
  ```

- `show-icons` defaults to `false`.
- Static `icon` metadata is valid even when `show-icons` is omitted or false.
- `show-icons=#true` applies uniformly to all menu levels and search results.
- Missing or unresolved icons render as a blank reserved icon slot so labels align.
- When `show-icons=#false`, no icon slot is rendered.
- Static icon names must be non-empty and must not be path-like.
- Invalid installed-app icon metadata is treated as missing, not as a configuration error.
- Search results use the matched entry's own icon, not breadcrumb ancestor icons.
- Submenu rows may have icons and should use a trailing chevron affordance instead of appending `" >"` to the label.

## Implementation Notes

- Add `show_icons: bool` to `MenuConfig`.
- Add `icon: Option<String>` to `MenuItemConfig`.
- Parse `show-icons=#true|#false` from the `menu` node header.
- Parse at most one child `icon "name"` node on static menu items.
- Reject duplicate static `icon` nodes.
- Reject empty or path-like static icon names.
- Keep `icon` display-only; it must not affect action, submenu, or inert item behavior.
- Extend `MenuEntryView` with `icon: Option<String>`.
- Populate static menu entries from `MenuItemConfig.icon`.
- Populate installed-app menu entries from `DesktopApplication.icon`, after filtering invalid/path-like values.
- Extract tray icon-name validation and XDG icon-theme lookup into a shared internal helper such as `crates/niribar/src/icon_theme.rs`.
- Keep tray-specific fallback behavior inside tray rendering.
- Panel menu rendering should use the shared resolver, but apply panel-specific blank-slot fallback.
- Replace text suffix submenu rendering with a trailing chevron in the row layout.

## Acceptance Criteria

- Existing panel menu configs continue to render text-only by default.
- A menu with `show-icons=#true` renders an icon column for all visible entries.
- Static item icons render when resolvable by XDG icon name.
- Installed-app icons render when their desktop metadata supplies a resolvable XDG icon name.
- Missing or unresolved icons reserve blank icon space and do not show a fallback glyph.
- Search results preserve breadcrumb labels and use the matched entry's own icon.
- Submenu entries render any configured icon plus a trailing chevron.
- Path-like static icon values fail configuration parsing.
- Path-like installed-app icon values are ignored.

## Suggested Tests

- Parser accepts `menu "main" show-icons=#true { ... }`.
- Parser defaults `show_icons` to false.
- Parser accepts `icon "utilities-terminal"` on static items with actions, children, or neither.
- Parser rejects duplicate `icon` nodes.
- Parser rejects empty and path-like static icon names.
- Installed-app entry generation carries valid app icon names.
- Installed-app entry generation drops path-like app icon names.
- Static search results use the matched item's icon.
- Menu render model exposes `show_icons` and entry icons consistently across submenu levels and search results.
- Existing panel menu tests continue to pass with default text-only rendering.

## Comments

- Created from a `grill-with-docs` design session on 2026-05-22.
- Implemented with TDD on 2026-05-22. Verification: `cargo test` passed with 776 tests.

## Implementation review — 2026-09-23

Implementation is present in crates/niribar/src/app/panel.rs; src/surface/panel.rs. Evidence: panel menu icon tests; graphical inspection remains pending.

Routed to human acceptance review to avoid reimplementing an existing feature.
The original acceptance boxes are retained; this routing change does not certify
manual acceptance. See [the remediation record](../../../../docs/shell/audits/2026-09-23-remediation.md)
for known limits and remaining checks.
