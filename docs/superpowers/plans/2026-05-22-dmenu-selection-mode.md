# Dmenu Selection Mode Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add `niribar --dmenu` as a dmenu-style one-shot selector that reads stdin, asks the running Niribar instance to show a panel menu, and writes the selected value to stdout.

**Architecture:** Keep `toggle-panel\n` as the legacy line IPC command, and add structured framed IPC for request/response features. The CLI parses dmenu options and stdin into a bounded `DmenuRequest`; the daemon displays that request through the existing panel menu machinery using a request-owned dynamic source; activation or cancellation completes the IPC response.

**Tech Stack:** Rust 2024, Tokio Unix sockets, `serde`/`serde_json` for framed IPC payloads, Iced/Iced Layershell panel rendering, existing KDL config parser tests.

---

## File Structure

- `Cargo.toml`: add direct `serde` and `serde_json` dependencies.
- `src/ipc/protocol.rs`: new pure protocol module with request/response types, delimiter escape parsing, stdin splitting, payload limits, and length-prefixed JSON frame helpers.
- `src/ipc.rs`: keep socket path and legacy line command support; add framed request handling and response writing.
- `src/cli.rs`: parse `--dmenu`, `-p`/`--prompt`, `--delimiter`, `-d`, and `--no-custom`.
- `src/main.rs`: route dmenu CLI mode: read stdin, build bounded request, send framed IPC, print result, map exit codes.
- `src/config/module.rs`: add `prompt: Option<String>` to `MenuConfig` for general panel menu prompt metadata.
- `src/config/parse/mod.rs`: parse optional `prompt="..."` menu property if cheap in the same slice.
- `src/app/messages.rs`: add daemon-side dmenu IPC and panel events.
- `src/app/panel.rs`: store pending dmenu request state, expose dmenu entries through the existing panel menu entry pipeline, complete requests on activation/cancel/focus loss.
- `src/app/render.rs`: add prompt/custom affordance fields to `PanelRenderModel`.
- `src/app/view.rs`: render prompt as search placeholder/header and render a custom-value row when allowed.
- `src/surface/panel.rs`: add request-owned menu identity to `PanelContent` without pretending it is configured menu content.
- `src/config/tests.rs`, `src/config/parse/tests.rs`, `src/ipc.rs`, `src/cli.rs`, `src/app/panel.rs`, `src/app/view.rs`: focused tests per slice.

## Protocol Rules

- Legacy client command: `toggle-panel\n`.
- Framed request magic: `NIRIBAR_IPC_JSON_V1\n`, followed by an 8-byte big-endian payload length and UTF-8 JSON.
- Initial framed payloads:

```rust
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum IpcRequest {
    DmenuSelection(DmenuRequest),
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct DmenuRequest {
    pub prompt: Option<String>,
    pub entries: Vec<String>,
    pub allow_custom: bool,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum IpcResponse {
    Selected { value: String },
    Cancelled,
    Rejected { reason: String },
}
```

- Limits for v1:
  - max frame bytes: `1_048_576`
  - max entries: `10_000`
  - max one entry bytes: `16_384`

## Task 1: Add Protocol Module and Dependencies

**Files:**
- Modify: `Cargo.toml`
- Modify: `src/ipc.rs`
- Create: `src/ipc/protocol.rs`

- [ ] **Step 1: Add failing protocol tests**

Add `src/ipc/protocol.rs` with these tests first:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decode_delimiter_escapes_common_sequences() {
        assert_eq!(decode_delimiter(r"\n").unwrap(), "\n");
        assert_eq!(decode_delimiter(r"\t").unwrap(), "\t");
        assert_eq!(decode_delimiter(r"\0").unwrap(), "\0");
        assert_eq!(decode_delimiter(r"\\").unwrap(), "\\");
        assert_eq!(decode_delimiter(r",").unwrap(), ",");
    }

    #[test]
    fn split_entries_preserves_duplicates_and_empty_input() {
        assert_eq!(
            split_entries("one\ntwo\none", "\n").unwrap(),
            vec!["one".to_string(), "two".to_string(), "one".to_string()]
        );
        assert_eq!(split_entries("", "\n").unwrap(), Vec::<String>::new());
    }

    #[test]
    fn split_entries_rejects_empty_delimiter() {
        let err = split_entries("abc", "").unwrap_err();
        assert_eq!(err, ProtocolError::EmptyDelimiter);
    }

    #[test]
    fn validate_request_rejects_too_many_entries() {
        let req = DmenuRequest {
            prompt: None,
            entries: vec!["x".to_string(); MAX_DMENU_ENTRIES + 1],
            allow_custom: true,
        };
        assert_eq!(
            validate_dmenu_request(&req).unwrap_err(),
            ProtocolError::TooManyEntries
        );
    }

    #[test]
    fn frame_roundtrip_preserves_dmenu_request() {
        let request = IpcRequest::DmenuSelection(DmenuRequest {
            prompt: Some("Run".to_string()),
            entries: vec!["firefox".to_string(), "kitty".to_string()],
            allow_custom: false,
        });
        let frame = encode_frame(&request).unwrap();
        assert!(frame.starts_with(FRAMED_MAGIC.as_bytes()));
        assert_eq!(decode_frame_payload(&frame).unwrap(), request);
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run:

```bash
cargo test ipc::protocol
```

Expected: FAIL because `src/ipc/protocol.rs` is not wired into `src/ipc.rs` and protocol types/functions are missing.

- [ ] **Step 3: Add dependencies and implementation**

Add to `Cargo.toml`:

```toml
serde = { version = "1", features = ["derive"] }
serde_json = "1"
```

At the top of `src/ipc.rs`, add:

```rust
pub(crate) mod protocol;
```

Create `src/ipc/protocol.rs`:

```rust
pub(crate) const FRAMED_MAGIC: &str = "NIRIBAR_IPC_JSON_V1\n";
pub(crate) const MAX_FRAME_BYTES: usize = 1_048_576;
pub(crate) const MAX_DMENU_ENTRIES: usize = 10_000;
pub(crate) const MAX_DMENU_ENTRY_BYTES: usize = 16_384;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ProtocolError {
    EmptyDelimiter,
    UnknownEscape(char),
    TooManyEntries,
    EntryTooLarge,
    FrameTooLarge,
    FrameTooShort,
    MissingMagic,
    Json(String),
}

impl std::fmt::Display for ProtocolError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyDelimiter => write!(f, "delimiter must not be empty"),
            Self::UnknownEscape(ch) => write!(f, "unknown delimiter escape \\{ch}"),
            Self::TooManyEntries => write!(f, "too many dmenu entries"),
            Self::EntryTooLarge => write!(f, "dmenu entry is too large"),
            Self::FrameTooLarge => write!(f, "IPC frame is too large"),
            Self::FrameTooShort => write!(f, "IPC frame is incomplete"),
            Self::MissingMagic => write!(f, "missing IPC frame magic"),
            Self::Json(err) => write!(f, "invalid IPC JSON: {err}"),
        }
    }
}

impl std::error::Error for ProtocolError {}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub(crate) enum IpcRequest {
    DmenuSelection(DmenuRequest),
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub(crate) struct DmenuRequest {
    pub(crate) prompt: Option<String>,
    pub(crate) entries: Vec<String>,
    pub(crate) allow_custom: bool,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub(crate) enum IpcResponse {
    Selected { value: String },
    Cancelled,
    Rejected { reason: String },
}

pub(crate) fn decode_delimiter(input: &str) -> Result<String, ProtocolError> {
    let mut out = String::new();
    let mut chars = input.chars();
    while let Some(ch) = chars.next() {
        if ch != '\\' {
            out.push(ch);
            continue;
        }
        let Some(escaped) = chars.next() else {
            return Err(ProtocolError::UnknownEscape('\\'));
        };
        match escaped {
            'n' => out.push('\n'),
            't' => out.push('\t'),
            '0' => out.push('\0'),
            '\\' => out.push('\\'),
            other => return Err(ProtocolError::UnknownEscape(other)),
        }
    }
    if out.is_empty() {
        return Err(ProtocolError::EmptyDelimiter);
    }
    Ok(out)
}

pub(crate) fn split_entries(input: &str, delimiter: &str) -> Result<Vec<String>, ProtocolError> {
    if delimiter.is_empty() {
        return Err(ProtocolError::EmptyDelimiter);
    }
    if input.is_empty() {
        return Ok(Vec::new());
    }
    let entries = input
        .split(delimiter)
        .filter(|entry| !entry.is_empty())
        .map(str::to_string)
        .collect::<Vec<_>>();
    Ok(entries)
}

pub(crate) fn validate_dmenu_request(request: &DmenuRequest) -> Result<(), ProtocolError> {
    if request.entries.len() > MAX_DMENU_ENTRIES {
        return Err(ProtocolError::TooManyEntries);
    }
    if request
        .entries
        .iter()
        .any(|entry| entry.len() > MAX_DMENU_ENTRY_BYTES)
    {
        return Err(ProtocolError::EntryTooLarge);
    }
    Ok(())
}

pub(crate) fn encode_frame<T: serde::Serialize>(value: &T) -> Result<Vec<u8>, ProtocolError> {
    let payload =
        serde_json::to_vec(value).map_err(|err| ProtocolError::Json(err.to_string()))?;
    if payload.len() > MAX_FRAME_BYTES {
        return Err(ProtocolError::FrameTooLarge);
    }
    let mut frame = Vec::with_capacity(FRAMED_MAGIC.len() + 8 + payload.len());
    frame.extend_from_slice(FRAMED_MAGIC.as_bytes());
    frame.extend_from_slice(&(payload.len() as u64).to_be_bytes());
    frame.extend_from_slice(&payload);
    Ok(frame)
}

pub(crate) fn decode_frame_payload(frame: &[u8]) -> Result<IpcRequest, ProtocolError> {
    if !frame.starts_with(FRAMED_MAGIC.as_bytes()) {
        return Err(ProtocolError::MissingMagic);
    }
    let len_start = FRAMED_MAGIC.len();
    let len_end = len_start + 8;
    if frame.len() < len_end {
        return Err(ProtocolError::FrameTooShort);
    }
    let mut len_bytes = [0_u8; 8];
    len_bytes.copy_from_slice(&frame[len_start..len_end]);
    let payload_len = u64::from_be_bytes(len_bytes) as usize;
    if payload_len > MAX_FRAME_BYTES {
        return Err(ProtocolError::FrameTooLarge);
    }
    let payload_end = len_end + payload_len;
    if frame.len() < payload_end {
        return Err(ProtocolError::FrameTooShort);
    }
    serde_json::from_slice(&frame[len_end..payload_end])
        .map_err(|err| ProtocolError::Json(err.to_string()))
}
```

- [ ] **Step 4: Run test to verify it passes**

Run:

```bash
cargo test ipc::protocol
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml Cargo.lock src/ipc.rs src/ipc/protocol.rs
git commit -m "feat: add framed IPC protocol for dmenu requests"
```

## Task 2: Parse Dmenu CLI Options

**Files:**
- Modify: `src/cli.rs`

- [ ] **Step 1: Add failing CLI parser tests**

Extend `src/cli.rs` tests:

```rust
#[test]
fn dmenu_defaults_to_newline_delimiter_and_custom_values() {
    match parse_args(["--dmenu".to_string()]) {
        Command::Dmenu(options) => {
            assert_eq!(options.prompt, None);
            assert_eq!(options.delimiter, "\n");
            assert!(options.allow_custom);
        }
        _ => panic!("expected dmenu command"),
    }
}

#[test]
fn dmenu_accepts_prompt_aliases_and_no_custom() {
    match parse_args([
        "--dmenu".to_string(),
        "-p".to_string(),
        "Run".to_string(),
        "--no-custom".to_string(),
    ]) {
        Command::Dmenu(options) => {
            assert_eq!(options.prompt.as_deref(), Some("Run"));
            assert!(!options.allow_custom);
        }
        _ => panic!("expected dmenu command"),
    }

    match parse_args([
        "--dmenu".to_string(),
        "--prompt".to_string(),
        "Launch".to_string(),
    ]) {
        Command::Dmenu(options) => assert_eq!(options.prompt.as_deref(), Some("Launch")),
        _ => panic!("expected dmenu command"),
    }
}

#[test]
fn dmenu_accepts_delimiter_aliases() {
    match parse_args([
        "--dmenu".to_string(),
        "--delimiter".to_string(),
        r"\0".to_string(),
    ]) {
        Command::Dmenu(options) => assert_eq!(options.delimiter, "\0"),
        _ => panic!("expected dmenu command"),
    }

    match parse_args(["--dmenu".to_string(), "-d".to_string(), ",".to_string()]) {
        Command::Dmenu(options) => assert_eq!(options.delimiter, ","),
        _ => panic!("expected dmenu command"),
    }
}

#[test]
fn dmenu_reports_missing_option_value() {
    match parse_args(["--dmenu".to_string(), "--prompt".to_string()]) {
        Command::UsageError(message) => assert!(message.contains("--prompt")),
        _ => panic!("expected usage error"),
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run:

```bash
cargo test cli
```

Expected: FAIL because `Command::Dmenu`, `DmenuOptions`, and usage errors do not exist.

- [ ] **Step 3: Implement parser**

Replace the command enum and parser body in `src/cli.rs` with:

```rust
#[allow(clippy::enum_variant_names)]
pub enum Command {
    Run,
    PrintVersion,
    IpcCommand(String),
    Dmenu(DmenuOptions),
    UsageError(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DmenuOptions {
    pub prompt: Option<String>,
    pub delimiter: String,
    pub allow_custom: bool,
}

impl Default for DmenuOptions {
    fn default() -> Self {
        Self {
            prompt: None,
            delimiter: "\n".into(),
            allow_custom: true,
        }
    }
}

pub fn parse() -> Command {
    parse_args(std::env::args().skip(1))
}

fn parse_args(args: impl IntoIterator<Item = String>) -> Command {
    let mut args = args.into_iter().peekable();
    let mut dmenu = false;
    let mut dmenu_options = DmenuOptions::default();

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--version" | "-V" => return Command::PrintVersion,
            "--toggle-panel" => return Command::IpcCommand("toggle-panel".into()),
            "--dmenu" => dmenu = true,
            "-p" | "--prompt" => {
                let Some(value) = args.next() else {
                    return Command::UsageError(format!("{arg} requires a value"));
                };
                dmenu_options.prompt = Some(value);
            }
            "-d" | "--delimiter" => {
                let Some(value) = args.next() else {
                    return Command::UsageError(format!("{arg} requires a value"));
                };
                match crate::ipc::protocol::decode_delimiter(&value) {
                    Ok(delimiter) => dmenu_options.delimiter = delimiter,
                    Err(err) => return Command::UsageError(err.to_string()),
                }
            }
            "--no-custom" => dmenu_options.allow_custom = false,
            _ => {}
        }
    }

    if dmenu {
        Command::Dmenu(dmenu_options)
    } else {
        Command::Run
    }
}
```

- [ ] **Step 4: Run test to verify it passes**

Run:

```bash
cargo test cli
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/cli.rs
git commit -m "feat: parse dmenu CLI options"
```

## Task 3: Add Client-Side Dmenu Request/Response

**Files:**
- Modify: `src/main.rs`
- Modify: `src/ipc.rs`

- [ ] **Step 1: Add focused unit tests for response exit mapping**

In `src/main.rs` test module, add:

```rust
#[test]
fn dmenu_response_exit_codes_are_stable() {
    assert_eq!(
        super::exit_code_for_dmenu_response(&crate::ipc::protocol::IpcResponse::Selected {
            value: "x".into()
        }),
        0
    );
    assert_eq!(
        super::exit_code_for_dmenu_response(&crate::ipc::protocol::IpcResponse::Cancelled),
        1
    );
    assert_eq!(super::DMENU_USAGE_EXIT, 2);
    assert_eq!(super::DMENU_REQUEST_FAILURE_EXIT, 3);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run:

```bash
cargo test main::tests::dmenu_response_exit_codes_are_stable
```

Expected: FAIL because constants and helper do not exist.

- [ ] **Step 3: Implement client helpers**

In `src/main.rs`, add near `send_ipc_command`:

```rust
const DMENU_USAGE_EXIT: i32 = 2;
const DMENU_REQUEST_FAILURE_EXIT: i32 = 3;

fn exit_code_for_dmenu_response(response: &ipc::protocol::IpcResponse) -> i32 {
    match response {
        ipc::protocol::IpcResponse::Selected { .. } => 0,
        ipc::protocol::IpcResponse::Cancelled => 1,
        ipc::protocol::IpcResponse::Rejected { .. } => DMENU_REQUEST_FAILURE_EXIT,
    }
}

fn run_dmenu_cli(options: cli::DmenuOptions) -> i32 {
    use std::io::{Read, Write};

    let mut input = String::new();
    if let Err(err) = std::io::stdin().read_to_string(&mut input) {
        eprintln!("niribar: failed to read stdin: {err}");
        return DMENU_USAGE_EXIT;
    }

    let entries = match ipc::protocol::split_entries(&input, &options.delimiter) {
        Ok(entries) => entries,
        Err(err) => {
            eprintln!("niribar: {err}");
            return DMENU_USAGE_EXIT;
        }
    };
    let request = ipc::protocol::DmenuRequest {
        prompt: options.prompt,
        entries,
        allow_custom: options.allow_custom,
    };
    if let Err(err) = ipc::protocol::validate_dmenu_request(&request) {
        eprintln!("niribar: {err}");
        return DMENU_USAGE_EXIT;
    }

    let response = match send_dmenu_request(&request) {
        Ok(response) => response,
        Err(err) => {
            eprintln!("niribar: dmenu request failed: {err}");
            return DMENU_REQUEST_FAILURE_EXIT;
        }
    };

    if let ipc::protocol::IpcResponse::Selected { value } = &response {
        println!("{value}");
        let _ = std::io::stdout().flush();
    } else if let ipc::protocol::IpcResponse::Rejected { reason } = &response {
        eprintln!("niribar: dmenu request rejected: {reason}");
    }

    exit_code_for_dmenu_response(&response)
}

fn send_dmenu_request(
    request: &ipc::protocol::DmenuRequest,
) -> anyhow::Result<ipc::protocol::IpcResponse> {
    use std::io::{Read, Write};
    use std::os::unix::net::UnixStream;

    let path = ipc::socket_path()?;
    let mut stream = UnixStream::connect(&path)?;
    let frame = ipc::protocol::encode_frame(&ipc::protocol::IpcRequest::DmenuSelection(
        request.clone(),
    ))?;
    stream.write_all(&frame)?;
    stream.shutdown(std::net::Shutdown::Write)?;

    let mut response_bytes = Vec::new();
    stream.read_to_end(&mut response_bytes)?;
    ipc::decode_response_frame(&response_bytes)
}
```

In `main()`, add the match arm:

```rust
cli::Command::Dmenu(options) => std::process::exit(run_dmenu_cli(options)),
cli::Command::UsageError(message) => {
    eprintln!("niribar: {message}");
    std::process::exit(DMENU_USAGE_EXIT);
}
```

In `src/ipc.rs`, add:

```rust
pub(crate) fn decode_response_frame(
    frame: &[u8],
) -> anyhow::Result<protocol::IpcResponse> {
    if !frame.starts_with(protocol::FRAMED_MAGIC.as_bytes()) {
        anyhow::bail!("missing response frame magic");
    }
    let len_start = protocol::FRAMED_MAGIC.len();
    let len_end = len_start + 8;
    if frame.len() < len_end {
        anyhow::bail!("response frame is incomplete");
    }
    let mut len_bytes = [0_u8; 8];
    len_bytes.copy_from_slice(&frame[len_start..len_end]);
    let payload_len = u64::from_be_bytes(len_bytes) as usize;
    let payload_end = len_end + payload_len;
    if payload_len > protocol::MAX_FRAME_BYTES || frame.len() < payload_end {
        anyhow::bail!("invalid response frame length");
    }
    Ok(serde_json::from_slice(&frame[len_end..payload_end])?)
}
```

- [ ] **Step 4: Run test to verify it passes**

Run:

```bash
cargo test main::tests::dmenu_response_exit_codes_are_stable
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/main.rs src/ipc.rs
git commit -m "feat: send dmenu requests from CLI"
```

## Task 4: Add Daemon Framed IPC Handling

**Files:**
- Modify: `src/ipc.rs`
- Modify: `src/app/messages.rs`
- Modify: `src/app.rs`

- [ ] **Step 1: Add failing IPC parser tests**

In `src/ipc.rs` tests, add:

```rust
#[test]
fn framed_dmenu_request_decodes_to_ipc_message() {
    let request = protocol::IpcRequest::DmenuSelection(protocol::DmenuRequest {
        prompt: Some("Run".into()),
        entries: vec!["firefox".into()],
        allow_custom: true,
    });
    let frame = protocol::encode_frame(&request).unwrap();
    let decoded = ipc_message_for_bytes(&frame).expect("message");

    assert!(matches!(
        decoded,
        Message::Ipc(crate::app::messages::IpcEvent::DmenuSelection { .. })
    ));
}

#[test]
fn legacy_toggle_panel_bytes_still_work() {
    let decoded = ipc_message_for_bytes(b"toggle-panel\n").expect("message");
    assert!(matches!(
        decoded,
        Message::Surface(crate::app::messages::SurfaceEvent::TogglePanel)
    ));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run:

```bash
cargo test ipc::tests::framed_dmenu_request_decodes_to_ipc_message ipc::tests::legacy_toggle_panel_bytes_still_work
```

Expected: FAIL because `Message::Ipc`, `IpcEvent`, and byte parser do not exist.

- [ ] **Step 3: Implement daemon message plumbing**

In `src/app/messages.rs`, add:

```rust
#[allow(dead_code)]
#[derive(Debug)]
pub enum IpcEvent {
    DmenuSelection {
        request: crate::ipc::protocol::DmenuRequest,
        responder: tokio::sync::oneshot::Sender<crate::ipc::protocol::IpcResponse>,
    },
}
```

In `src/app.rs`, add enum variant:

```rust
Ipc(messages::IpcEvent),
```

Add to `perf_label`:

```rust
Message::Ipc(_) => "Ipc",
```

Add to `update`:

```rust
Message::Ipc(event) => self.handle_ipc_event(event),
```

In `src/ipc.rs`, replace the accept-loop body with a byte-read path:

```rust
let mut stream = stream;
let mut bytes = Vec::new();
if tokio::io::AsyncReadExt::read_to_end(&mut stream, &mut bytes).await.is_ok()
    && let Some(message) = ipc_message_for_bytes(&bytes)
    && tx.send(message).is_err()
{
    break;
}
```

Add parser helpers:

```rust
fn ipc_message_for_bytes(bytes: &[u8]) -> Option<Message> {
    if bytes.starts_with(protocol::FRAMED_MAGIC.as_bytes()) {
        let request = protocol::decode_frame_payload(bytes).ok()?;
        return ipc_message_for_request(request);
    }
    let line = std::str::from_utf8(bytes).ok()?;
    ipc_message_for_line(line)
}

fn ipc_message_for_request(request: protocol::IpcRequest) -> Option<Message> {
    match request {
        protocol::IpcRequest::DmenuSelection(request) => {
            let (tx, rx) = tokio::sync::oneshot::channel();
            Some(Message::Ipc(crate::app::messages::IpcEvent::DmenuSelection {
                request,
                responder: tx,
            }))
        }
    }
}
```

Important: after creating `rx`, the accept task must write the response back to the same stream. Use this structure in the accept loop:

```rust
if bytes.starts_with(protocol::FRAMED_MAGIC.as_bytes()) {
    if let Ok(protocol::IpcRequest::DmenuSelection(request)) =
        protocol::decode_frame_payload(&bytes)
    {
        let (responder, response_rx) = tokio::sync::oneshot::channel();
        if tx
            .send(Message::Ipc(crate::app::messages::IpcEvent::DmenuSelection {
                request,
                responder,
            }))
            .is_ok()
            && let Ok(response) = response_rx.await
            && let Ok(frame) = protocol::encode_frame(&response)
        {
            let _ = tokio::io::AsyncWriteExt::write_all(&mut stream, &frame).await;
        }
    }
    continue;
}
```

- [ ] **Step 4: Run test to verify it passes**

Run:

```bash
cargo test ipc::tests::framed_dmenu_request_decodes_to_ipc_message ipc::tests::legacy_toggle_panel_bytes_still_work
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/ipc.rs src/app.rs src/app/messages.rs
git commit -m "feat: receive framed dmenu IPC requests"
```

## Task 5: Add Panel Menu Prompt Metadata

**Files:**
- Modify: `src/config/module.rs`
- Modify: `src/config/parse/mod.rs`
- Modify: `src/config/parse/tests.rs`
- Modify: `src/app/render.rs`
- Modify: `src/app/view.rs`

- [ ] **Step 1: Add failing config parser test**

In `src/config/parse/tests.rs`, add:

```rust
#[test]
fn parse_menu_with_prompt_metadata() {
    let kdl = r#"
menus {
    menu "apps" prompt="Launch" {
        source "installed-apps"
    }
}
"#;
    let config = parse(kdl).unwrap();
    assert_eq!(config.menus[0].prompt.as_deref(), Some("Launch"));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run:

```bash
cargo test config::parse::tests::parse_menu_with_prompt_metadata
```

Expected: FAIL because `MenuConfig::prompt` does not exist and `prompt` is an unknown menu property.

- [ ] **Step 3: Implement prompt metadata**

In `src/config/module.rs`, change `MenuConfig` to:

```rust
pub struct MenuConfig {
    pub name: String,
    pub show_icons: bool,
    pub prompt: Option<String>,
    pub source: Option<MenuSourceConfig>,
    pub items: Vec<MenuItemConfig>,
}
```

In `src/config/parse/mod.rs`, add:

```rust
let prompt = parse_menu_prompt(node, &menu_ctx)?;
```

and include it when pushing `MenuConfig`:

```rust
menus.push(MenuConfig {
    name,
    show_icons,
    prompt,
    source,
    items,
});
```

Add helper:

```rust
fn parse_menu_prompt(node: &KdlNode, ctx: &ParseCtx) -> ConfigResult<Option<String>> {
    let mut prompt = None;
    for entry in node.entries().iter().filter(|entry| entry.name().is_some()) {
        if entry.name().map(|name| name.value()) != Some("prompt") {
            continue;
        }
        let value = entry
            .value()
            .as_string()
            .ok_or_else(|| ctx.entry_err(entry, "expected prompt string", None))?;
        prompt = Some(value.to_string());
    }
    Ok(prompt
        .filter(|value| !value.is_empty()))
}
```

Update `parse_menu_show_icons` to allow `prompt` without error:

```rust
Some("prompt") => {}
```

Update all `MenuConfig` struct literals in tests to include:

```rust
prompt: None,
```

In `src/app/render.rs`, add to `PanelRenderModel`:

```rust
pub(super) menu_prompt: Option<String>,
pub(super) allow_custom_value: bool,
```

Set configured prompt in `panel_render_model`:

```rust
let menu_prompt = match &tree.content {
    crate::surface::panel::PanelContent::Menu(name) => self
        .config
        .menus
        .iter()
        .find(|menu| menu.name == *name)
        .and_then(|menu| menu.prompt.clone()),
    _ => None,
};
```

For now set:

```rust
allow_custom_value: false,
```

In `src/app/view.rs`, use prompt as placeholder:

```rust
let placeholder = model.menu_prompt.as_deref().unwrap_or("Search");
let search = text_input(placeholder, &model.search_query)
```

If a header is desired in this slice, render:

```rust
let content = if let Some(prompt) = model.menu_prompt.clone() {
    column![text(prompt), search, menu_scroll]
} else {
    column![search, menu_scroll]
};
container(content)
```

- [ ] **Step 4: Run tests**

Run:

```bash
cargo test config::parse::tests::parse_menu_with_prompt_metadata
cargo test config::tests
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/config/module.rs src/config/parse/mod.rs src/config/parse/tests.rs src/config/tests.rs src/app/render.rs src/app/view.rs
git commit -m "feat: add panel menu prompt metadata"
```

## Task 6: Model Request-Owned Dmenu Panel Menus

**Files:**
- Modify: `src/surface/panel.rs`
- Modify: `src/app/panel.rs`
- Modify: `src/app/render.rs`

- [ ] **Step 1: Add failing panel entry tests**

In `src/app/panel.rs` tests, add:

```rust
#[test]
fn dmenu_entries_use_input_as_label_and_value() {
    let request = crate::ipc::protocol::DmenuRequest {
        prompt: Some("Run".into()),
        entries: vec!["firefox".into(), "kitty".into(), "firefox".into()],
        allow_custom: true,
    };
    let entries = dmenu_entry_views(&request, "");

    assert_eq!(
        entries.iter().map(|entry| entry.label.as_str()).collect::<Vec<_>>(),
        vec!["firefox", "kitty", "firefox"]
    );
    assert!(matches!(entries[0].target, MenuEntryTarget::DmenuValue(_)));
}

#[test]
fn dmenu_search_filters_entries_case_insensitively() {
    let request = crate::ipc::protocol::DmenuRequest {
        prompt: None,
        entries: vec!["Firefox".into(), "Kitty".into()],
        allow_custom: true,
    };
    let entries = dmenu_entry_views(&request, "fire");
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].label, "Firefox");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run:

```bash
cargo test app::panel::tests::dmenu_entries_use_input_as_label_and_value app::panel::tests::dmenu_search_filters_entries_case_insensitively
```

Expected: FAIL because dmenu targets and helper do not exist.

- [ ] **Step 3: Add dmenu content and entry target**

In `src/surface/panel.rs`, extend `PanelContent`:

```rust
DmenuRequest,
```

In `src/app/panel.rs`, extend `PanelSystem`:

```rust
pub(super) dmenu_request: Option<crate::ipc::protocol::DmenuRequest>,
pub(super) dmenu_responder: Option<tokio::sync::oneshot::Sender<crate::ipc::protocol::IpcResponse>>,
```

Extend `MenuEntryTarget`:

```rust
DmenuValue(String),
```

Add helper:

```rust
pub(super) fn dmenu_entry_views(
    request: &crate::ipc::protocol::DmenuRequest,
    query: &str,
) -> Vec<MenuEntryView> {
    let query = query.to_ascii_lowercase();
    request
        .entries
        .iter()
        .enumerate()
        .filter(|(_, value)| {
            query.is_empty() || value.to_ascii_lowercase().contains(&query)
        })
        .map(|(index, value)| MenuEntryView {
            label: value.clone(),
            icon: None,
            path: vec![index],
            target: MenuEntryTarget::DmenuValue(value.clone()),
            is_submenu: false,
        })
        .collect()
}
```

Update menu-content matches in `src/app/panel.rs`, `src/app/render.rs`, and `src/app/view.rs` so `PanelContent::DmenuRequest` is treated as menu content alongside `PanelContent::Menu(_)`.

In `menu_entries_for_tree_and_menu`, add a separate dmenu path before configured menu lookup:

```rust
if matches!(tree.content, crate::surface::panel::PanelContent::DmenuRequest) {
    return self
        .panel
        .dmenu_request
        .as_ref()
        .map(|request| dmenu_entry_views(request, &self.panel.search_query))
        .unwrap_or_default();
}
```

If the existing function requires a configured `MenuConfig`, split it into:

```rust
fn active_menu_tree_and_entries(&self) -> Option<(crate::surface::panel::PanelTree, Vec<MenuEntryView>)>
```

that handles `DmenuRequest` first, then configured menus.

- [ ] **Step 4: Run tests**

Run:

```bash
cargo test app::panel::tests::dmenu_entries_use_input_as_label_and_value app::panel::tests::dmenu_search_filters_entries_case_insensitively
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/surface/panel.rs src/app/panel.rs src/app/render.rs src/app/view.rs
git commit -m "feat: model dmenu requests as panel menu content"
```

## Task 7: Open Dmenu Requests from IPC and Complete Responses

**Files:**
- Modify: `src/app.rs`
- Modify: `src/app/panel.rs`
- Modify: `src/app/messages.rs`
- Modify: `src/app/surfaces.rs`

- [ ] **Step 1: Add failing app behavior tests**

In `src/app/panel.rs` tests, add:

```rust
#[test]
fn dmenu_request_rejects_when_another_request_is_pending() {
    let (mut app, _task) = crate::app::App::boot(crate::config::Config::default(), Vec::new());
    app.panel.dmenu_request = Some(crate::ipc::protocol::DmenuRequest {
        prompt: None,
        entries: vec![],
        allow_custom: true,
    });
    let (tx, rx) = tokio::sync::oneshot::channel();

    let _ = app.handle_ipc_event(crate::app::messages::IpcEvent::DmenuSelection {
        request: crate::ipc::protocol::DmenuRequest {
            prompt: None,
            entries: vec!["x".into()],
            allow_custom: true,
        },
        responder: tx,
    });

    let response = rx.try_recv().unwrap();
    assert!(matches!(response, crate::ipc::protocol::IpcResponse::Rejected { .. }));
}

#[test]
fn completing_dmenu_selection_sends_selected_response() {
    let (mut app, _task) = crate::app::App::boot(crate::config::Config::default(), Vec::new());
    let (tx, mut rx) = tokio::sync::oneshot::channel();
    app.panel.dmenu_request = Some(crate::ipc::protocol::DmenuRequest {
        prompt: None,
        entries: vec!["firefox".into()],
        allow_custom: true,
    });
    app.panel.dmenu_responder = Some(tx);

    app.complete_dmenu_request(crate::ipc::protocol::IpcResponse::Selected {
        value: "firefox".into(),
    });

    assert_eq!(
        rx.try_recv().unwrap(),
        crate::ipc::protocol::IpcResponse::Selected {
            value: "firefox".into()
        }
    );
    assert!(app.panel.dmenu_request.is_none());
}
```

- [ ] **Step 2: Run test to verify it fails**

Run:

```bash
cargo test app::panel::tests::dmenu_request_rejects_when_another_request_is_pending app::panel::tests::completing_dmenu_selection_sends_selected_response
```

Expected: FAIL because `handle_ipc_event` and completion helper do not exist or are private/missing.

- [ ] **Step 3: Implement IPC event handling**

In `src/app.rs`, add:

```rust
pub(super) fn handle_ipc_event(&mut self, event: messages::IpcEvent) -> Task<Message> {
    match event {
        messages::IpcEvent::DmenuSelection { request, responder } => {
            self.handle_dmenu_selection_request(request, responder)
        }
    }
}
```

In `src/app/panel.rs`, add:

```rust
pub(super) fn handle_dmenu_selection_request(
    &mut self,
    request: crate::ipc::protocol::DmenuRequest,
    responder: tokio::sync::oneshot::Sender<crate::ipc::protocol::IpcResponse>,
) -> Task<Message> {
    if self.panel.dmenu_request.is_some() {
        let _ = responder.send(crate::ipc::protocol::IpcResponse::Rejected {
            reason: "another dmenu request is already active".into(),
        });
        return Task::none();
    }

    let Some(origin_window) = self.default_panel_origin_window() else {
        let _ = responder.send(crate::ipc::protocol::IpcResponse::Rejected {
            reason: "no bar surface is available".into(),
        });
        return Task::none();
    };

    let tasks = self.close_all_panels();
    self.panel.clear();
    self.panel.dmenu_request = Some(request);
    self.panel.dmenu_responder = Some(responder);
    self.panel.focused_index = Some(0);

    Task::batch(
        tasks
            .into_iter()
            .chain(std::iter::once(self.handle_panel_event(
                super::messages::PanelEvent::Open {
                    origin_window,
                    origin: crate::surface::panel::PanelOrigin::Bar,
                    content: crate::surface::panel::PanelContent::DmenuRequest,
                    anchor: None,
                },
            ))),
    )
}

pub(super) fn complete_dmenu_request(
    &mut self,
    response: crate::ipc::protocol::IpcResponse,
) {
    if let Some(responder) = self.panel.dmenu_responder.take() {
        let _ = responder.send(response);
    }
    self.panel.dmenu_request = None;
}

fn default_panel_origin_window(&self) -> Option<iced::window::Id> {
    self.surfaces.keys().copied().next()
}
```

If `SurfaceManager::keys()` does not exist, add a small iterator method following the existing `panel_popup_ids()` style.

In `activate_panel_entry`, add a dmenu target branch:

```rust
MenuEntryTarget::DmenuValue(value) => {
    self.complete_dmenu_request(crate::ipc::protocol::IpcResponse::Selected { value });
    Task::batch(self.close_all_panels())
}
```

In panel close paths (`PanelEvent::Close`, focus-lost close handling), before clearing the panel, call:

```rust
self.complete_dmenu_request(crate::ipc::protocol::IpcResponse::Cancelled);
```

Guard this so non-dmenu panels do not send anything:

```rust
if self.panel.dmenu_request.is_some() {
    self.complete_dmenu_request(crate::ipc::protocol::IpcResponse::Cancelled);
}
```

- [ ] **Step 4: Run tests**

Run:

```bash
cargo test app::panel::tests::dmenu_request_rejects_when_another_request_is_pending app::panel::tests::completing_dmenu_selection_sends_selected_response
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/app.rs src/app/panel.rs src/app/messages.rs src/app/surfaces.rs
git commit -m "feat: open and complete dmenu panel requests"
```

## Task 8: Add Custom Value Affordance

**Files:**
- Modify: `src/app/panel.rs`
- Modify: `src/app/render.rs`
- Modify: `src/app/view.rs`
- Modify: `src/app/messages.rs`

- [ ] **Step 1: Add failing custom value tests**

In `src/app/panel.rs` tests, add:

```rust
#[test]
fn dmenu_custom_value_entry_appears_when_query_has_no_exact_match() {
    let request = crate::ipc::protocol::DmenuRequest {
        prompt: None,
        entries: vec!["firefox".into()],
        allow_custom: true,
    };
    let entries = dmenu_entry_views(&request, "kitty");

    assert_eq!(entries[0].label, "kitty");
    assert!(matches!(entries[0].target, MenuEntryTarget::DmenuCustomValue(_)));
}

#[test]
fn dmenu_no_custom_does_not_create_custom_entry() {
    let request = crate::ipc::protocol::DmenuRequest {
        prompt: None,
        entries: vec!["firefox".into()],
        allow_custom: false,
    };
    let entries = dmenu_entry_views(&request, "kitty");

    assert!(entries.is_empty());
}
```

- [ ] **Step 2: Run test to verify it fails**

Run:

```bash
cargo test app::panel::tests::dmenu_custom_value_entry_appears_when_query_has_no_exact_match app::panel::tests::dmenu_no_custom_does_not_create_custom_entry
```

Expected: FAIL because custom target does not exist.

- [ ] **Step 3: Implement custom value entry**

Extend `MenuEntryTarget`:

```rust
DmenuCustomValue(String),
```

Update `dmenu_entry_views` so it prepends custom value when allowed and query is non-empty:

```rust
let mut entries = Vec::new();
let exact_match = request.entries.iter().any(|entry| entry == query);
if request.allow_custom && !query.is_empty() && !exact_match {
    entries.push(MenuEntryView {
        label: query.to_string(),
        icon: None,
        path: Vec::new(),
        target: MenuEntryTarget::DmenuCustomValue(query.to_string()),
        is_submenu: false,
    });
}
entries.extend(/* filtered DmenuValue entries from Task 6 */);
entries
```

Update activation:

```rust
MenuEntryTarget::DmenuValue(value) | MenuEntryTarget::DmenuCustomValue(value) => {
    self.complete_dmenu_request(crate::ipc::protocol::IpcResponse::Selected { value });
    Task::batch(self.close_all_panels())
}
```

No separate keyboard shortcut is needed in v1 because the custom value is a distinct first row; normal activation still returns the focused entry.

- [ ] **Step 4: Run tests**

Run:

```bash
cargo test app::panel::tests::dmenu_custom_value_entry_appears_when_query_has_no_exact_match app::panel::tests::dmenu_no_custom_does_not_create_custom_entry
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/app/panel.rs src/app/render.rs src/app/view.rs src/app/messages.rs
git commit -m "feat: allow custom dmenu values"
```

## Task 9: End-to-End Verification and Cleanup

**Files:**
- Modify as needed only for compile/test fixes from earlier tasks.

- [ ] **Step 1: Run formatting**

Run:

```bash
cargo fmt --all -- --check
```

Expected: PASS. If it fails, run `cargo fmt --all`, then rerun the check.

- [ ] **Step 2: Run full tests**

Run:

```bash
cargo test
```

Expected: PASS.

- [ ] **Step 3: Run clippy**

Run:

```bash
cargo clippy -- -D warnings
```

Expected: PASS.

- [ ] **Step 4: Manual smoke test with a running daemon**

Start or use an existing Niribar daemon, then run:

```bash
printf 'firefox\nkitty\ncode\n' | cargo run -- --dmenu -p Run
```

Expected: the panel menu opens on the focused output; selecting `kitty` prints:

```text
kitty
```

Run:

```bash
printf 'firefox\0kitty\0' | cargo run -- --dmenu --delimiter '\0' --no-custom
```

Expected: two entries appear; arbitrary unmatched text cannot be submitted.

- [ ] **Step 5: Commit verification fixes**

```bash
git add Cargo.toml Cargo.lock src docs
git commit -m "test: verify dmenu selection mode"
```

Skip this commit if Task 9 made no file changes.

## Self-Review

- Spec coverage: Covers CLI parsing, stdin delimiter handling, prompt metadata, no daemon fallback, bounded request payload, request/response IPC, panel-menu reuse, custom values by default, `--no-custom`, duplicate preservation, empty input behavior, output newline normalization, cancellation/failure distinction, no timeout, focus-loss cancellation, and concurrent request rejection.
- Placeholder scan: The implementation tasks include concrete paths, commands, expected outcomes, and code snippets. The only conditional instruction is for adding a `SurfaceManager::keys()` iterator if the existing API is absent; it is bounded and points to the local pattern to follow.
- Type consistency: `DmenuRequest`, `IpcRequest`, `IpcResponse`, `DmenuOptions`, `PanelContent::DmenuRequest`, and `MenuEntryTarget::{DmenuValue,DmenuCustomValue}` are introduced before later tasks use them.
