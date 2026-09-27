# Rich Dmenu Entries Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Extend dmenu selection mode so callers can pass JSON Lines rich entries with labels, returned values, icons, and keywords while preserving existing plain dmenu behavior.

**Architecture:** The IPC protocol will carry `DmenuEntry` objects instead of raw strings; plain input is converted to entries where label and value are identical. Rich JSONL parsing lives at the CLI/protocol boundary, and panel rendering consumes a normalized entry shape. The panel remains a leaf selector that returns values to stdout and never runs entry-owned actions.

**Tech Stack:** Rust, serde/serde_json, existing IPC framed JSON protocol, existing `cargo test` unit tests.

---

## File Structure

- Modify `crates/niribar/src/ipc/protocol.rs`: add normalized `DmenuEntry`, entry-format parsing helpers, validation, and serialization tests.
- Modify `crates/niribar/src/cli.rs`: add dmenu entry-format CLI options and reject custom delimiters with JSONL mode.
- Modify `crates/niribar/src/main.rs`: parse stdin according to selected format and build `DmenuRequest`.
- Modify `crates/niribar/src/app/panel.rs`: render rich labels/icons/keywords, return rich values, preserve duplicates, and enable request-scoped icons.
- Modify `crates/niribar/src/app/render.rs` or the nearby render model builder if required: make dmenu panel menu model set `show_icons` when any request entry has an icon.
- Tests stay colocated in existing `#[cfg(test)]` modules.

## Public Behavior

Plain mode remains default:

```bash
printf 'firefox\nkitty\n' | niribar --dmenu
```

Rich mode is explicit:

```bash
printf '%s\n' \
  '{"label":"Firefox","value":"org.mozilla.firefox","icon":"firefox","keywords":["browser","web"]}' \
  '{"value":"kitty"}' \
  | niribar --dmenu --entry-format jsonl
```

Accepted aliases:

```bash
niribar --dmenu --json
niribar --dmenu --entry-format jsonl
niribar --dmenu --entry-format plain
```

Rejected:

```bash
niribar --dmenu --entry-format jsonl --delimiter ','
niribar --dmenu --json -d ','
```

## Task 1: Add Protocol Types and Plain Conversion

**Files:**
- Modify: `crates/niribar/src/ipc/protocol.rs`

- [ ] **Step 1: Write failing protocol tests**

Add these tests inside `crates/niribar/src/ipc/protocol.rs` `mod tests`:

```rust
#[test]
fn split_plain_entries_converts_to_dmenu_entries() {
    let entries = split_dmenu_entries_plain("firefox\nkitty\nfirefox", "\n").unwrap();

    assert_eq!(
        entries,
        vec![
            DmenuEntry::plain("firefox"),
            DmenuEntry::plain("kitty"),
            DmenuEntry::plain("firefox"),
        ]
    );
}

#[test]
fn frame_roundtrip_preserves_rich_dmenu_entries() {
    let request = IpcRequest::DmenuSelection(DmenuRequest {
        prompt: Some("Run".to_string()),
        entries: vec![
            DmenuEntry {
                label: Some("Firefox".to_string()),
                value: Some("org.mozilla.firefox".to_string()),
                icon: Some("firefox".to_string()),
                keywords: vec!["browser".to_string(), "web".to_string()],
            },
            DmenuEntry {
                label: None,
                value: Some("kitty".to_string()),
                icon: None,
                keywords: Vec::new(),
            },
        ],
        allow_custom: false,
    });

    let frame = encode_frame(&request).unwrap();
    assert_eq!(decode_frame_payload(&frame).unwrap(), request);
}
```

- [ ] **Step 2: Run protocol tests and confirm they fail**

Run:

```bash
cargo test ipc::protocol::tests::split_plain_entries_converts_to_dmenu_entries ipc::protocol::tests::frame_roundtrip_preserves_rich_dmenu_entries
```

Expected: fail to compile because `DmenuEntry` and `split_dmenu_entries_plain` do not exist and `DmenuRequest.entries` is still `Vec<String>`.

- [ ] **Step 3: Implement `DmenuEntry` and plain conversion**

In `crates/niribar/src/ipc/protocol.rs`, replace `DmenuRequest.entries: Vec<String>` with `Vec<DmenuEntry>` and add:

```rust
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct DmenuEntry {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    #[serde(default)]
    pub keywords: Vec<String>,
}

impl DmenuEntry {
    pub(crate) fn plain(value: impl Into<String>) -> Self {
        let value = value.into();
        Self {
            label: Some(value.clone()),
            value: Some(value),
            icon: None,
            keywords: Vec::new(),
        }
    }

    pub(crate) fn display_label(&self) -> &str {
        self.label
            .as_deref()
            .or(self.value.as_deref())
            .unwrap_or("")
    }

    pub(crate) fn selected_value(&self) -> &str {
        self.value
            .as_deref()
            .or(self.label.as_deref())
            .unwrap_or("")
    }
}
```

Add:

```rust
pub(crate) fn split_dmenu_entries_plain(
    input: &str,
    delimiter: &str,
) -> Result<Vec<DmenuEntry>, ProtocolError> {
    split_entries(input, delimiter).map(|entries| {
        entries
            .into_iter()
            .map(DmenuEntry::plain)
            .collect::<Vec<_>>()
    })
}
```

Update existing protocol tests that build `DmenuRequest` to use `DmenuEntry::plain("x")` instead of strings.

- [ ] **Step 4: Run protocol tests**

Run:

```bash
cargo test ipc::protocol::tests
```

Expected: protocol tests compile and pass, or fail only on validation tests to be addressed in Task 2.

- [ ] **Step 5: Commit**

```bash
rtk git add crates/niribar/src/ipc/protocol.rs
rtk git commit -m "feat: add dmenu entry protocol type"
```

## Task 2: Add Rich JSONL Parsing and Validation

**Files:**
- Modify: `crates/niribar/src/ipc/protocol.rs`

- [ ] **Step 1: Write failing JSONL and validation tests**

Add these tests inside `crates/niribar/src/ipc/protocol.rs` `mod tests`:

```rust
#[test]
fn parse_rich_dmenu_entries_from_json_lines() {
    let input = concat!(
        r#"{"label":"Firefox","value":"org.mozilla.firefox","icon":"firefox","keywords":["browser","web"]}"#,
        "\n",
        r#"{"value":"kitty"}"#,
        "\n"
    );

    let entries = parse_dmenu_entries_jsonl(input).unwrap();

    assert_eq!(entries[0].display_label(), "Firefox");
    assert_eq!(entries[0].selected_value(), "org.mozilla.firefox");
    assert_eq!(entries[0].icon.as_deref(), Some("firefox"));
    assert_eq!(entries[0].keywords, vec!["browser", "web"]);
    assert_eq!(entries[1].display_label(), "kitty");
    assert_eq!(entries[1].selected_value(), "kitty");
}

#[test]
fn parse_rich_dmenu_entries_rejects_unknown_fields() {
    let err = parse_dmenu_entries_jsonl(r#"{"label":"Firefox","extra":true}"#).unwrap_err();
    assert!(matches!(err, ProtocolError::Json(_)));
}

#[test]
fn validate_request_rejects_rich_entry_without_label_or_value() {
    let req = DmenuRequest {
        prompt: None,
        entries: vec![DmenuEntry {
            label: None,
            value: None,
            icon: None,
            keywords: Vec::new(),
        }],
        allow_custom: true,
    };

    assert_eq!(
        validate_dmenu_request(&req).unwrap_err(),
        ProtocolError::EmptyDmenuEntry
    );
}

#[test]
fn validate_request_rejects_invalid_rich_icon_names() {
    let req = DmenuRequest {
        prompt: None,
        entries: vec![DmenuEntry {
            label: Some("Firefox".to_string()),
            value: Some("firefox".to_string()),
            icon: Some("/usr/share/icons/firefox.png".to_string()),
            keywords: Vec::new(),
        }],
        allow_custom: true,
    };

    assert_eq!(
        validate_dmenu_request(&req).unwrap_err(),
        ProtocolError::InvalidDmenuIcon
    );
}
```

- [ ] **Step 2: Run tests and confirm they fail**

Run:

```bash
cargo test ipc::protocol::tests::parse_rich_dmenu_entries_from_json_lines ipc::protocol::tests::parse_rich_dmenu_entries_rejects_unknown_fields ipc::protocol::tests::validate_request_rejects_rich_entry_without_label_or_value ipc::protocol::tests::validate_request_rejects_invalid_rich_icon_names
```

Expected: fail to compile because `parse_dmenu_entries_jsonl`, `EmptyDmenuEntry`, and `InvalidDmenuIcon` do not exist.

- [ ] **Step 3: Implement parser and validation errors**

In `ProtocolError`, add variants:

```rust
EmptyDmenuEntry,
InvalidDmenuIcon,
```

In `Display`, add:

```rust
Self::EmptyDmenuEntry => write!(f, "rich dmenu entry must include label or value"),
Self::InvalidDmenuIcon => write!(f, "rich dmenu entry icon must be an XDG icon name"),
```

Add:

```rust
pub(crate) fn parse_dmenu_entries_jsonl(input: &str) -> Result<Vec<DmenuEntry>, ProtocolError> {
    input
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            serde_json::from_str::<DmenuEntry>(line)
                .map_err(|err| ProtocolError::Json(err.to_string()))
        })
        .collect()
}
```

Update `validate_dmenu_request`:

```rust
pub(crate) fn validate_dmenu_request(request: &DmenuRequest) -> Result<(), ProtocolError> {
    if request.entries.len() > MAX_DMENU_ENTRIES {
        return Err(ProtocolError::TooManyEntries);
    }
    for entry in &request.entries {
        if entry.display_label().is_empty() && entry.selected_value().is_empty() {
            return Err(ProtocolError::EmptyDmenuEntry);
        }
        if entry.display_label().len() > MAX_DMENU_ENTRY_BYTES
            || entry.selected_value().len() > MAX_DMENU_ENTRY_BYTES
            || entry.keywords.iter().any(|keyword| keyword.len() > MAX_DMENU_ENTRY_BYTES)
        {
            return Err(ProtocolError::EntryTooLarge);
        }
        if let Some(icon) = &entry.icon {
            if !crate::icon_theme::is_valid_icon_name(icon) {
                return Err(ProtocolError::InvalidDmenuIcon);
            }
        }
    }
    Ok(())
}
```

- [ ] **Step 4: Run protocol tests**

Run:

```bash
cargo test ipc::protocol::tests
```

Expected: all protocol tests pass.

- [ ] **Step 5: Commit**

```bash
rtk git add crates/niribar/src/ipc/protocol.rs
rtk git commit -m "feat: parse rich dmenu json lines"
```

## Task 3: Add CLI Entry Format Options

**Files:**
- Modify: `crates/niribar/src/cli.rs`

- [ ] **Step 1: Write failing CLI tests**

Add these tests inside `crates/niribar/src/cli.rs` `mod tests`:

```rust
#[test]
fn dmenu_defaults_to_plain_entry_format() {
    match parse_args(["--dmenu".to_string()]) {
        Command::Dmenu(options) => assert_eq!(options.entry_format, DmenuEntryFormat::Plain),
        _ => panic!("expected dmenu command"),
    }
}

#[test]
fn dmenu_accepts_json_entry_format_aliases() {
    match parse_args(["--dmenu".to_string(), "--json".to_string()]) {
        Command::Dmenu(options) => assert_eq!(options.entry_format, DmenuEntryFormat::JsonLines),
        _ => panic!("expected dmenu command"),
    }

    match parse_args([
        "--dmenu".to_string(),
        "--entry-format".to_string(),
        "jsonl".to_string(),
    ]) {
        Command::Dmenu(options) => assert_eq!(options.entry_format, DmenuEntryFormat::JsonLines),
        _ => panic!("expected dmenu command"),
    }
}

#[test]
fn dmenu_accepts_plain_entry_format() {
    match parse_args([
        "--dmenu".to_string(),
        "--entry-format".to_string(),
        "plain".to_string(),
    ]) {
        Command::Dmenu(options) => assert_eq!(options.entry_format, DmenuEntryFormat::Plain),
        _ => panic!("expected dmenu command"),
    }
}

#[test]
fn dmenu_rejects_json_entry_format_with_custom_delimiter() {
    match parse_args([
        "--dmenu".to_string(),
        "--json".to_string(),
        "--delimiter".to_string(),
        ",".to_string(),
    ]) {
        Command::UsageError(message) => assert!(message.contains("delimiter")),
        _ => panic!("expected usage error"),
    }
}
```

- [ ] **Step 2: Run CLI tests and confirm they fail**

Run:

```bash
cargo test cli::tests::dmenu_defaults_to_plain_entry_format cli::tests::dmenu_accepts_json_entry_format_aliases cli::tests::dmenu_accepts_plain_entry_format cli::tests::dmenu_rejects_json_entry_format_with_custom_delimiter
```

Expected: fail to compile because `DmenuEntryFormat` and `entry_format` do not exist.

- [ ] **Step 3: Implement CLI format parsing**

In `crates/niribar/src/cli.rs`, add:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DmenuEntryFormat {
    Plain,
    JsonLines,
}
```

Add this field to `DmenuOptions`:

```rust
pub entry_format: DmenuEntryFormat,
```

Update `Default`:

```rust
entry_format: DmenuEntryFormat::Plain,
```

In `parse_args`, track custom delimiter:

```rust
let mut custom_delimiter = false;
```

Inside delimiter parsing after successful decode:

```rust
custom_delimiter = true;
```

Add match arms:

```rust
"--json" => dmenu_options.entry_format = DmenuEntryFormat::JsonLines,
"--entry-format" => {
    let Some(value) = args.next() else {
        return Command::UsageError(format!("{arg} requires a value"));
    };
    match value.as_str() {
        "plain" => dmenu_options.entry_format = DmenuEntryFormat::Plain,
        "jsonl" | "json-lines" => dmenu_options.entry_format = DmenuEntryFormat::JsonLines,
        _ => return Command::UsageError(format!("unknown dmenu entry format {value:?}")),
    }
}
```

Before returning `Command::Dmenu(dmenu_options)`:

```rust
if dmenu
    && custom_delimiter
    && matches!(dmenu_options.entry_format, DmenuEntryFormat::JsonLines)
{
    return Command::UsageError(
        "dmenu delimiter options cannot be combined with JSONL entry format".into(),
    );
}
```

- [ ] **Step 4: Run CLI tests**

Run:

```bash
cargo test cli::tests
```

Expected: all CLI tests pass.

- [ ] **Step 5: Commit**

```bash
rtk git add crates/niribar/src/cli.rs
rtk git commit -m "feat: add dmenu entry format option"
```

## Task 4: Wire Main CLI Input Parsing

**Files:**
- Modify: `crates/niribar/src/main.rs`

- [ ] **Step 1: Write failing main tests**

Add this helper near the dmenu code in `crates/niribar/src/main.rs`:

```rust
fn parse_dmenu_stdin(
    input: &str,
    options: &cli::DmenuOptions,
) -> Result<Vec<ipc::protocol::DmenuEntry>, ipc::protocol::ProtocolError> {
    match options.entry_format {
        cli::DmenuEntryFormat::Plain => {
            ipc::protocol::split_dmenu_entries_plain(input, &options.delimiter)
        }
        cli::DmenuEntryFormat::JsonLines => ipc::protocol::parse_dmenu_entries_jsonl(input),
    }
}
```

First add it only as a signature if needed to write tests, then add tests in `crates/niribar/src/main.rs` `mod tests`:

```rust
#[test]
fn parse_dmenu_stdin_uses_plain_format_by_default() {
    let options = crate::cli::DmenuOptions::default();
    let entries = super::parse_dmenu_stdin("firefox\nkitty\n", &options).unwrap();

    assert_eq!(
        entries,
        vec![
            crate::ipc::protocol::DmenuEntry::plain("firefox"),
            crate::ipc::protocol::DmenuEntry::plain("kitty"),
        ]
    );
}

#[test]
fn parse_dmenu_stdin_uses_json_lines_format() {
    let mut options = crate::cli::DmenuOptions::default();
    options.entry_format = crate::cli::DmenuEntryFormat::JsonLines;

    let entries = super::parse_dmenu_stdin(
        r#"{"label":"Firefox","value":"org.mozilla.firefox","icon":"firefox"}"#,
        &options,
    )
    .unwrap();

    assert_eq!(entries[0].display_label(), "Firefox");
    assert_eq!(entries[0].selected_value(), "org.mozilla.firefox");
}
```

- [ ] **Step 2: Run tests and confirm they fail**

Run:

```bash
cargo test main::tests::parse_dmenu_stdin_uses_plain_format_by_default main::tests::parse_dmenu_stdin_uses_json_lines_format
```

Expected: fail until helper exists and `run_dmenu_cli` uses normalized entries.

- [ ] **Step 3: Implement helper and update `run_dmenu_cli`**

In `run_dmenu_cli`, replace:

```rust
let entries = match ipc::protocol::split_entries(&input, &options.delimiter) {
```

with:

```rust
let entries = match parse_dmenu_stdin(&input, &options) {
```

Keep the existing error handling, request creation, and validation flow.

- [ ] **Step 4: Run main tests**

Run:

```bash
cargo test main::tests
```

Expected: all main tests pass.

- [ ] **Step 5: Commit**

```bash
rtk git add crates/niribar/src/main.rs
rtk git commit -m "feat: parse dmenu stdin by entry format"
```

## Task 5: Render Rich Entries in the Panel

**Files:**
- Modify: `crates/niribar/src/app/panel.rs`

- [ ] **Step 1: Write failing panel tests**

Add these tests inside `crates/niribar/src/app/panel.rs` `mod tests`:

```rust
#[test]
fn dmenu_rich_entries_use_label_icon_keywords_and_value() {
    let request = crate::ipc::protocol::DmenuRequest {
        prompt: Some("Run".into()),
        entries: vec![
            crate::ipc::protocol::DmenuEntry {
                label: Some("Firefox".into()),
                value: Some("org.mozilla.firefox".into()),
                icon: Some("firefox".into()),
                keywords: vec!["browser".into(), "web".into()],
            },
            crate::ipc::protocol::DmenuEntry {
                label: Some("Kitty".into()),
                value: Some("kitty".into()),
                icon: None,
                keywords: Vec::new(),
            },
        ],
        allow_custom: true,
    };

    let entries = dmenu_entry_views(&request, "browser");

    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].label, "browser");
    assert!(matches!(entries[0].target, MenuEntryTarget::DmenuCustomValue(_)));
    assert_eq!(entries[1].label, "Firefox");
    assert_eq!(entries[1].icon.as_deref(), Some("firefox"));
    assert_eq!(
        entries[1].target,
        MenuEntryTarget::DmenuValue("org.mozilla.firefox".into())
    );
}

#[test]
fn dmenu_rich_search_does_not_match_opaque_value_when_label_exists() {
    let request = crate::ipc::protocol::DmenuRequest {
        prompt: None,
        entries: vec![crate::ipc::protocol::DmenuEntry {
            label: Some("Firefox".into()),
            value: Some("org.mozilla.firefox".into()),
            icon: None,
            keywords: Vec::new(),
        }],
        allow_custom: false,
    };

    let entries = dmenu_entry_views(&request, "mozilla");

    assert!(entries.is_empty());
}

#[test]
fn dmenu_value_only_rich_entry_matches_and_displays_value() {
    let request = crate::ipc::protocol::DmenuRequest {
        prompt: None,
        entries: vec![crate::ipc::protocol::DmenuEntry {
            label: None,
            value: Some("org.mozilla.firefox".into()),
            icon: None,
            keywords: Vec::new(),
        }],
        allow_custom: false,
    };

    let entries = dmenu_entry_views(&request, "mozilla");

    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].label, "org.mozilla.firefox");
}
```

- [ ] **Step 2: Run panel tests and confirm they fail**

Run:

```bash
cargo test app::panel::tests::dmenu_rich_entries_use_label_icon_keywords_and_value app::panel::tests::dmenu_rich_search_does_not_match_opaque_value_when_label_exists app::panel::tests::dmenu_value_only_rich_entry_matches_and_displays_value
```

Expected: fail because `dmenu_entry_views` still treats entries like strings.

- [ ] **Step 3: Update `dmenu_entry_views`**

Replace the exact-match calculation and mapping in `dmenu_entry_views` with entry-aware logic:

```rust
let exact_match = request
    .entries
    .iter()
    .any(|entry| entry.display_label() == query || entry.selected_value() == query);
```

Use a helper:

```rust
fn dmenu_entry_matches(entry: &crate::ipc::protocol::DmenuEntry, query: &str) -> bool {
    if query.is_empty() {
        return true;
    }
    let label = entry.display_label().to_ascii_lowercase();
    label.contains(query)
        || entry
            .keywords
            .iter()
            .any(|keyword| keyword.to_ascii_lowercase().contains(query))
}
```

Then map:

```rust
.filter(|(_, entry)| dmenu_entry_matches(entry, &query))
.map(|(index, entry)| MenuEntryView {
    label: entry.display_label().to_string(),
    icon: entry.icon.clone(),
    path: vec![index],
    target: MenuEntryTarget::DmenuValue(entry.selected_value().to_string()),
    is_submenu: false,
})
```

- [ ] **Step 4: Update old dmenu tests to construct `DmenuEntry::plain`**

Every existing `DmenuRequest { entries: vec!["firefox".into()] }` in `crates/niribar/src/app/panel.rs` should become:

```rust
entries: vec![crate::ipc::protocol::DmenuEntry::plain("firefox")],
```

For duplicates:

```rust
entries: vec![
    crate::ipc::protocol::DmenuEntry::plain("firefox"),
    crate::ipc::protocol::DmenuEntry::plain("kitty"),
    crate::ipc::protocol::DmenuEntry::plain("firefox"),
],
```

- [ ] **Step 5: Run panel tests**

Run:

```bash
cargo test app::panel::tests
```

Expected: all panel tests pass or fail only on request-scoped icon rendering covered in Task 6.

- [ ] **Step 6: Commit**

```bash
rtk git add crates/niribar/src/app/panel.rs
rtk git commit -m "feat: render rich dmenu entries"
```

## Task 6: Enable Request-Scoped Dmenu Icons

**Files:**
- Modify: `crates/niribar/src/app/panel.rs`
- Modify: `crates/niribar/src/app/render.rs` only if the menu model is built there

- [ ] **Step 1: Locate current menu model construction**

Run:

```bash
rg -n "show_icons|MenuModel|DmenuRequest|active_menu_entries_for_render" crates/niribar/src/app
```

Expected: identify the branch that builds the panel menu model for `PanelContent::DmenuRequest`.

- [ ] **Step 2: Write failing icon-rendering model test**

Add a focused test near existing panel render/model tests. If the model construction is in `crates/niribar/src/app/render.rs`, place it there; otherwise keep it in `crates/niribar/src/app/panel.rs`.

Test intent:

```rust
#[test]
fn dmenu_request_with_entry_icon_enables_menu_icons() {
    let (mut app, _task) = crate::app::App::boot(crate::config::Config::default(), Vec::new());
    app.panel.dmenu_request = Some(crate::ipc::protocol::DmenuRequest {
        prompt: None,
        entries: vec![crate::ipc::protocol::DmenuEntry {
            label: Some("Firefox".into()),
            value: Some("firefox".into()),
            icon: Some("firefox".into()),
            keywords: Vec::new(),
        }],
        allow_custom: true,
    });

    let model = app
        .panel_menu_model_for_test(crate::surface::panel::PanelContent::DmenuRequest)
        .expect("dmenu model");

    assert!(model.show_icons);
}
```

If there is no suitable test helper, add a small `#[cfg(test)]` helper that returns the same model used by rendering instead of duplicating model-building logic.

- [ ] **Step 3: Run the new test and confirm it fails**

Run the exact test name chosen in Step 2:

```bash
cargo test dmenu_request_with_entry_icon_enables_menu_icons
```

Expected: fail because dmenu currently has no request-scoped icon rendering switch.

- [ ] **Step 4: Implement icon detection**

Add a helper near dmenu panel logic:

```rust
fn dmenu_request_show_icons(request: &crate::ipc::protocol::DmenuRequest) -> bool {
    request.entries.iter().any(|entry| entry.icon.is_some())
}
```

In the dmenu menu model branch, set:

```rust
show_icons: self
    .panel
    .dmenu_request
    .as_ref()
    .map(dmenu_request_show_icons)
    .unwrap_or(false),
```

Keep configured menus using their existing `menu.show_icons`.

- [ ] **Step 5: Run app tests**

Run:

```bash
cargo test app::panel::tests app::render::tests
```

Expected: all app panel/render tests pass.

- [ ] **Step 6: Commit**

```bash
rtk git add crates/niribar/src/app/panel.rs crates/niribar/src/app/render.rs
rtk git commit -m "feat: show icons for rich dmenu requests"
```

## Task 7: Update Remaining Call Sites and Run Full Verification

**Files:**
- Modify: any Rust files still constructing `DmenuRequest` with `Vec<String>`

- [ ] **Step 1: Find stale string-entry call sites**

Run:

```bash
rg -n "DmenuRequest \\{|entries: vec!\\[\"|entries: vec!\\[\\]" src
```

Expected: every non-empty `entries` construction uses `DmenuEntry::plain(...)` or an explicit `DmenuEntry`.

- [ ] **Step 2: Fix stale call sites**

For each plain request:

```rust
entries: vec![
    crate::ipc::protocol::DmenuEntry::plain("firefox"),
    crate::ipc::protocol::DmenuEntry::plain("kitty"),
],
```

Empty requests remain:

```rust
entries: vec![],
```

- [ ] **Step 3: Format**

Run:

```bash
cargo fmt
```

Expected: no output or only normal rustfmt completion.

- [ ] **Step 4: Run full tests**

Run:

```bash
cargo test
```

Expected: all tests pass.

- [ ] **Step 5: Inspect docs and diffs**

Run:

```bash
git diff --stat
git diff -- CONTEXT.md docs/niribar/adr/0005-rich-dmenu-input-uses-json-lines.md crates/niribar/src/ipc/protocol.rs crates/niribar/src/cli.rs crates/niribar/src/main.rs crates/niribar/src/app/panel.rs crates/niribar/src/app/render.rs
```

Expected: code matches the resolved behavior in `CONTEXT.md` and ADR `0005`; no unrelated refactors.

- [ ] **Step 6: Commit final cleanup**

```bash
rtk git add src docs
rtk git commit -m "test: verify rich dmenu entries"
```

## Self-Review

- Spec coverage: JSONL input, plain compatibility, label/value split, keywords search, XDG icon validation, missing-theme icon behavior, leaf-only entries, custom typed values, duplicate preservation, delimiter incompatibility, request-scoped icon rendering, and ADR documentation are covered.
- Placeholder scan: no task relies on unspecified “appropriate” behavior; every test and implementation step names concrete functions, fields, and commands.
- Type consistency: `DmenuEntry`, `DmenuEntryFormat`, `DmenuRequest.entries: Vec<DmenuEntry>`, `display_label`, `selected_value`, `parse_dmenu_entries_jsonl`, and `split_dmenu_entries_plain` are introduced before downstream tasks use them.
