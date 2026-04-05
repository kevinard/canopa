# Story 6.4: Toggle Expand/Collapse via Keyboard

Status: review

## Story

As a **user navigating the bar with the keyboard**,
I want to toggle the bar's expand/collapse state from the keyboard,
so that I can reveal or hide module labels without reaching for the mouse.

## Acceptance Criteria

1. **Given** the bar is collapsed
   **When** `Message::ToggleExpand` fires (e.g., from the IPC socket)
   **Then** the bar begins expanding using the same `ExpandState::toggle()` animation as a pointer click (`BarClick`)

2. **Given** the bar is expanded
   **When** `Message::ToggleExpand` fires
   **Then** the bar begins collapsing using the same animation

3. **Given** the bar is mid-animation (Expanding or Collapsing)
   **When** `Message::ToggleExpand` fires
   **Then** the animation reverses from its current position (toggle model — UX-DR13)

4. **Given** `expand-trigger` is set to `hover`
   **When** `Message::ToggleExpand` fires
   **Then** the toggle still fires (the keybind is unconditional — unlike `BarClick`, which is a no-op for hover trigger)

5. **Given** a running niribar instance
   **When** `niribar --toggle-expand` is executed
   **Then** niribar sends the toggle command to the Unix socket and the running instance toggles expand/collapse

6. **Given** the user's niribar config contains `expand-keybind "Super+e"`
   **When** the config is loaded
   **Then** `config.bar.expand_keybind == Some("Super+e".to_string())`

7. **Given** no `expand-keybind` in config
   **When** the config is loaded
   **Then** `config.bar.expand_keybind == None`

8. **Given** niribar is built
   **When** `cargo clippy -- -D warnings` is run
   **Then** zero warnings

9. **Given** niribar is built
   **When** `cargo test` is run
   **Then** all 259 pre-existing tests pass and at least 10 new tests pass (≥ 269 total)

## Mechanism: Why Unix Socket

Wayland layer-shell keyboard events are only delivered when the surface has keyboard focus (`KeyboardInteractivity::OnDemand`). There is no standard Wayland API for global hotkeys from a client application. The correct approach for "works without keyboard focus" is:

1. niribar listens on a Unix socket at `$XDG_RUNTIME_DIR/niribar.sock`
2. The user binds a key in their Niri compositor config to run `niribar --toggle-expand`
3. `niribar --toggle-expand` sends `"toggle-expand\n"` to the socket and exits
4. The socket subscription emits `Message::ToggleExpand` into the Iced event loop
5. `config.bar.expand_keybind` is a **documentation field** — it records the intended keybind for user reference; niribar does not parse the key combination itself

## File Map

| File | Action | What changes |
|------|--------|-------------|
| `src/config/mod.rs` | **MODIFY** | Add `pub expand_keybind: Option<String>` to `BarConfig`; update `Default` impl |
| `src/config/parse.rs` | **MODIFY** | Parse `bar.expand-keybind` from KDL; add config tests |
| `src/app.rs` | **MODIFY** | Add `Message::ToggleExpand` variant; add update handler |
| `src/ipc.rs` | **NEW** | Unix socket subscription: `ipc_subscription()` returns `Subscription<Message>` |
| `src/main.rs` | **MODIFY** | Handle `--toggle-expand` CLI flag: write to socket and exit |

## Tasks / Subtasks

- [x] Task 1: Add `expand_keybind` field to `BarConfig` in `src/config/mod.rs` (AC: #6, #7)
  - [x] 1.1: Add field to struct:
    ```rust
    /// Keybind the user intends to configure in their compositor for expand/collapse.
    /// Documentation only — niribar does not intercept this key directly.
    /// Users bind this in their Niri config to run `niribar --toggle-expand`.
    pub expand_keybind: Option<String>,
    ```
  - [x] 1.2: Add `expand_keybind: None` to `BarConfig::default()`

- [x] Task 2: Parse `bar.expand-keybind` in `src/config/parse.rs` (AC: #6, #7)
  - [x] 2.1: Add parse logic inside `parse_bar_section()` after `hover_delay`:
    ```rust
    let expand_keybind = children
        .get_arg("expand-keybind")
        .and_then(|v| v.as_string())
        .map(|s| s.to_string());
    ```
  - [x] 2.2: Include `expand_keybind` in the returned `BarConfig { ... }` struct

- [x] Task 3: Add `Message::ToggleExpand` to `Message` enum in `src/app.rs` (AC: #1–4)
  - [x] 3.1: Add after `KeyboardActivate`:
    ```rust
    /// Keyboard-triggered expand/collapse toggle via Unix socket IPC (Story 6.4).
    /// Works regardless of bar focus state and expand_trigger setting.
    ToggleExpand,
    ```

- [x] Task 4: Add `ToggleExpand` handler in `App::update()` in `src/app.rs` (AC: #1–4)
  - [x] 4.1: Add before the `_ => Task::none()` catchall (after `KeyboardActivate`):
    ```rust
    Message::ToggleExpand => {
        // Keyboard keybind always toggles — no expand_trigger check (unlike BarClick).
        self.expand = self.expand.toggle();
        let w = self.expand.current_width(
            self.config.bar.collapsed_width,
            self.config.bar.expanded_width,
        );
        Task::batch([
            Task::done(Message::SizeChange((w, 0))),
            Task::done(Message::ExclusiveZoneChange(
                i32::try_from(w).unwrap_or(i32::MAX),
            )),
        ])
    }
    ```
    Pattern follows `Message::BarClick` handler — identical toggle + resize, minus the `expand_trigger` guard.

- [x] Task 5: Create `src/ipc.rs` — Unix socket subscription (AC: #5)
  - [x] 5.1: Create the module:
    ```rust
    use iced::futures::stream;
    use std::path::PathBuf;
    use tokio::io::AsyncBufReadExt;
    use tokio::net::UnixListener;

    use crate::app::Message;

    /// Path of the control socket. Uses `$XDG_RUNTIME_DIR/niribar.sock`.
    pub fn socket_path() -> PathBuf {
        let dir = std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/tmp".to_string());
        PathBuf::from(dir).join("niribar.sock")
    }

    /// Iced subscription that listens on the Unix socket for IPC commands.
    ///
    /// Emits `Message::ToggleExpand` when it receives `"toggle-expand"` on any connection.
    /// Removes stale socket file on startup. Runs indefinitely.
    pub fn ipc_subscription() -> iced::Subscription<Message> {
        iced::Subscription::run_with((), |()| {
            stream::unfold((), |()| async move {
                let path = socket_path();
                // Remove stale socket from a previous run.
                let _ = tokio::fs::remove_file(&path).await;
                let listener = match UnixListener::bind(&path) {
                    Ok(l) => l,
                    Err(_) => {
                        // Cannot bind — keep stream alive but emit nothing.
                        std::future::pending::<()>().await;
                        unreachable!()
                    }
                };
                loop {
                    let Ok((stream, _)) = listener.accept().await else {
                        continue;
                    };
                    let mut reader = tokio::io::BufReader::new(stream);
                    let mut line = String::new();
                    if reader.read_line(&mut line).await.is_ok() && line.trim() == "toggle-expand" {
                        return Some((Message::ToggleExpand, ()));
                    }
                }
            })
        })
    }
    ```
    Note: `stream::unfold` with `()` state loops by returning to the same state after each event, making the stream live as long as the app runs.

- [x] Task 6: Register `ipc_subscription()` in `App::subscription()` in `src/app.rs` (AC: #5)
  - [x] 6.1: Add `mod ipc;` to `src/main.rs` (not app.rs — module declarations go in main.rs)
  - [x] 6.2: Add `use crate::ipc::ipc_subscription;` at the top of `src/app.rs`
  - [x] 6.3: Include in `App::subscription()`:
    ```rust
    let ipc_sub = ipc_subscription();
    ```
    Add `ipc_sub` to the `iced::Subscription::batch([...])` call at the end of `subscription()`.

- [x] Task 7: Handle `--toggle-expand` CLI flag in `src/main.rs` (AC: #5)
  - [x] 7.1: Add before the `try_main()` call in `main()`:
    ```rust
    if std::env::args().any(|a| a == "--toggle-expand") {
        return send_ipc_command("toggle-expand");
    }
    ```
  - [x] 7.2: Add the helper function:
    ```rust
    fn send_ipc_command(cmd: &str) {
        use std::io::Write;
        use std::os::unix::net::UnixStream;
        let path = crate::ipc::socket_path();
        match UnixStream::connect(&path) {
            Ok(mut stream) => {
                let _ = writeln!(stream, "{cmd}");
            }
            Err(e) => {
                eprintln!("niribar: cannot connect to socket at {}: {e}", path.display());
                std::process::exit(1);
            }
        }
    }
    ```
    Note: uses `std::os::unix::net::UnixStream` (sync, no Tokio needed) since we just write and exit. Must add `mod ipc;` to `src/main.rs` so `crate::ipc::socket_path()` is accessible.

- [x] Task 8: Write tests (AC: #9)
  - [x] 8.1: Config tests in `src/config/parse.rs`:
    ```rust
    #[test]
    fn parse_expand_keybind() {
        let kdl = r#"bar { expand-keybind "Super+e" }"#;
        let config = parse(kdl).unwrap();
        assert_eq!(config.bar.expand_keybind, Some("Super+e".to_string()));
    }

    #[test]
    fn parse_expand_keybind_defaults_to_none() {
        let config = parse("").unwrap();
        assert_eq!(config.bar.expand_keybind, None);
    }

    #[test]
    fn parse_bar_without_expand_keybind_uses_none() {
        let kdl = r#"bar { edge "left" }"#;
        let config = parse(kdl).unwrap();
        assert!(config.bar.expand_keybind.is_none());
    }
    ```
  - [x] 8.2: ToggleExpand handler tests in `src/app.rs`:
    ```rust
    #[test]
    fn toggle_expand_from_collapsed_starts_expanding() {
        let mut app = App::new(make_config());
        assert_eq!(app.expand, ExpandState::Collapsed);
        let _ = app.update(Message::ToggleExpand);
        assert_eq!(app.expand, ExpandState::Expanding(0.0));
    }

    #[test]
    fn toggle_expand_from_expanded_starts_collapsing() {
        let mut app = App::new(make_config());
        app.expand = ExpandState::Expanded;
        let _ = app.update(Message::ToggleExpand);
        assert_eq!(app.expand, ExpandState::Collapsing(0.0));
    }

    #[test]
    fn toggle_expand_reverses_expanding_mid_animation() {
        let mut app = App::new(make_config());
        app.expand = ExpandState::Expanding(0.4);
        let _ = app.update(Message::ToggleExpand);
        assert!(
            matches!(app.expand, ExpandState::Collapsing(p) if (p - 0.6).abs() < 1e-5),
            "expected Collapsing(~0.6), got {:?}", app.expand
        );
    }

    #[test]
    fn toggle_expand_works_with_hover_trigger() {
        // Unlike BarClick (which is a no-op for Hover trigger), ToggleExpand always fires.
        let mut app = App::new(make_hover_config());
        assert_eq!(app.expand, ExpandState::Collapsed);
        let _ = app.update(Message::ToggleExpand);
        assert_eq!(app.expand, ExpandState::Expanding(0.0));
    }

    #[test]
    fn toggle_expand_double_toggle_returns_to_collapsed() {
        let mut app = App::new(make_config());
        let _ = app.update(Message::ToggleExpand); // → Expanding(0.0)
        // Simulate completion: manually set to Expanded
        app.expand = ExpandState::Expanded;
        let _ = app.update(Message::ToggleExpand); // → Collapsing(0.0)
        assert_eq!(app.expand, ExpandState::Collapsing(0.0));
    }

    #[test]
    fn toggle_expand_reverses_collapsing_mid_animation() {
        let mut app = App::new(make_config());
        app.expand = ExpandState::Collapsing(0.3);
        let _ = app.update(Message::ToggleExpand);
        assert!(
            matches!(app.expand, ExpandState::Expanding(p) if (p - 0.7).abs() < 1e-5),
            "expected Expanding(~0.7), got {:?}", app.expand
        );
    }
    ```
  - [x] 8.3: Config struct tests in `src/config/mod.rs`:
    ```rust
    #[test]
    fn bar_config_default_expand_keybind_is_none() {
        assert!(BarConfig::default().expand_keybind.is_none());
    }
    ```

## Critical Implementation Notes

### `mod ipc;` declaration goes in `src/main.rs`
Rust module declarations must be in the crate root or a parent module. Since `src/app.rs` is NOT the crate root, `mod ipc;` must go in `src/main.rs` (the crate root). `src/app.rs` then accesses it as `crate::ipc::ipc_subscription()`.

### Import in `src/app.rs`
Add at the top of `src/app.rs`:
```rust
use crate::ipc::ipc_subscription;
```

### `ipc_subscription()` subscription ID
`iced::Subscription::run_with((), ...)` uses `()` as the unique key. This means only one IPC subscription will ever be active (same key = same subscription). This is correct — we only want one socket listener.

### `stream::unfold` loop pattern in ipc.rs
The stream needs to loop on `listener.accept()` to handle multiple connections (each `--toggle-expand` call opens a new connection). The `loop` inside the `async move` block continues listening after each accepted connection. When `toggle-expand` is received, return `Some((Message::ToggleExpand, ()))` — the `()` state causes iced to call the stream function again from the `unfold`, but since we're using a loop, the listener is persistent.

Wait — actually `stream::unfold` restarts the closure on each iteration. The `UnixListener` would be recreated each time. We need to restructure to keep the listener alive across iterations.

**Corrected pattern for ipc.rs** — use a channel bridge:
```rust
pub fn ipc_subscription() -> iced::Subscription<Message> {
    iced::Subscription::run_with((), |()| {
        stream::unfold(None::<tokio::sync::mpsc::UnboundedReceiver<Message>>, |rx| async move {
            let mut rx = match rx {
                Some(r) => r,
                None => {
                    // First call: bind socket and spawn listener task.
                    let path = socket_path();
                    let _ = tokio::fs::remove_file(&path).await;
                    let Ok(listener) = UnixListener::bind(&path) else {
                        std::future::pending::<()>().await;
                        unreachable!()
                    };
                    let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
                    tokio::spawn(async move {
                        loop {
                            let Ok((stream, _)) = listener.accept().await else { continue };
                            let mut reader = tokio::io::BufReader::new(stream);
                            let mut line = String::new();
                            if reader.read_line(&mut line).await.is_ok()
                                && line.trim() == "toggle-expand"
                            {
                                if tx.send(Message::ToggleExpand).is_err() {
                                    break;
                                }
                            }
                        }
                    });
                    rx
                }
            };
            // Wait for next command from the listener task.
            match rx.recv().await {
                Some(msg) => Some((msg, Some(rx))),
                None => {
                    std::future::pending::<()>().await;
                    unreachable!()
                }
            }
        })
    })
}
```

This pattern matches how `niri_subscription` works: a background task handles the blocking/async I/O, sends events via a channel, and the stream yields from the channel. The `UnixListener` stays alive for the lifetime of the spawned task.

### `BarClick` vs `ToggleExpand` — no `expand_trigger` guard
`BarClick` returns early when `expand_trigger == ExpandTrigger::Hover`. `ToggleExpand` does NOT. The keyboard keybind is always active regardless of how pointer-based expand is configured.

### Socket cleanup on shutdown
The socket file is cleaned up on next startup (`tokio::fs::remove_file` before `bind`). There's no explicit cleanup on shutdown — this is intentional and matches how systemd sockets and other Unix tools behave.

### `Option<String>` derives for `BarConfig`
`BarConfig` already derives `Debug` and `Clone`. `Option<String>` satisfies both. No changes needed to derives.

### `PartialEq` for `BarConfig` in tests
The config tests compare `config.bar.expand_keybind`. `Option<String>` implements `PartialEq`. If `BarConfig` needs `PartialEq` (it doesn't derive it currently), add it or use `assert!(matches!(...))`. Prefer `assert_eq!` since `Option<String>: PartialEq`.

## Previous Story Learnings (Story 6.3)

- `mod ipc;` must be declared in `src/main.rs` (crate root), not `src/app.rs`
- Follow the `niri_subscription` channel-bridge pattern for async subscriptions — keep listener alive via `tokio::spawn` + `mpsc::unbounded_channel`
- New `Message` variants go after `KeyboardActivate` in the enum
- New handlers go before `_ => Task::none()` catchall
- `make_hover_config()` already exists in `src/app.rs` test helpers — reuse it for the hover-trigger test
- `make_config()` exists for default config — reuse for ToggleExpand basic tests

## Definition of Done

- [x] `BarConfig.expand_keybind: Option<String>` added with default `None`
- [x] `bar.expand-keybind` parsed from KDL config
- [x] `Message::ToggleExpand` added to `Message` enum
- [x] `ToggleExpand` handler toggles `self.expand` and issues resize (no expand_trigger guard)
- [x] `src/ipc.rs` created with `socket_path()` and `ipc_subscription()`
- [x] `ipc_subscription()` included in `App::subscription()`
- [x] `--toggle-expand` CLI flag sends to socket and exits
- [x] `cargo clippy -- -D warnings` clean
- [x] `cargo test` shows ≥ 269 tests passing (272 passing)

## Dev Agent Record

### Agent Model Used

claude-sonnet-4-6

### Debug Log References

None.

### Completion Notes List

- Used the channel-bridge pattern (matching `niri_subscription`) for `ipc_subscription()`: a `tokio::spawn` task owns the `UnixListener` and sends events via `mpsc::unbounded_channel`; `stream::unfold` yields from the channel. This keeps the listener alive across stream iterations.
- `expand_keybind` is documentation-only — niribar does not parse the key combination. Users bind their key in Niri compositor config to run `niribar --toggle-expand`.
- `ToggleExpand` handler has no `expand_trigger` guard (unlike `BarClick`). The keyboard toggle always fires regardless of hover/click trigger setting.
- Added `#[allow(dead_code)]` to `expand_keybind` field in `BarConfig` since it is stored but never read (serves as user-documentation in config).
- Fixed `collapsible_if` clippy lint in `ipc.rs` by collapsing nested `if` into single `&&` chain.
- Rust 2024 edition: `std::env::set_var`/`remove_var` are `unsafe`; wrapped in `unsafe { }` blocks with safety comments in `ipc.rs` tests.
- 272 tests pass (259 pre-existing + 13 new).

### File List

- `src/config/mod.rs` — Added `expand_keybind: Option<String>` to `BarConfig`; `#[allow(dead_code)]`; updated `Default`; 1 new test
- `src/config/parse.rs` — Parsed `expand-keybind` from KDL bar section; 3 new tests
- `src/app.rs` — Added `Message::ToggleExpand` variant; added `ToggleExpand` handler; added `use crate::ipc::ipc_subscription`; registered `ipc_subscription()` in `App::subscription()`; 6 new tests
- `src/ipc.rs` — NEW: `socket_path()`, `ipc_subscription()` with channel-bridge pattern; 3 unit tests
- `src/main.rs` — Added `mod ipc;`; added `--toggle-expand` CLI flag handling; added `send_ipc_command()` helper

## Change Log

| Date | Version | Description | Author |
|------|---------|-------------|--------|
| 2026-04-05 | 1.0 | Implemented story: Unix socket IPC (`src/ipc.rs`), `--toggle-expand` CLI flag, `Message::ToggleExpand` handler, `expand_keybind` config field. 272 tests passing. | claude-sonnet-4-6 |
