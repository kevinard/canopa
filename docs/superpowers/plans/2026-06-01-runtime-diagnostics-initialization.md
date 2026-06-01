# Runtime Diagnostics Initialization Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add the first runtime diagnostics slice: initialize `tracing`, preserve `NIRIBAR_PERF=1` behavior through tracing, and keep normal stdout/stderr behavior intact.

**Architecture:** A new root `diagnostics` module owns environment parsing and tracing subscriber setup. `main` initializes diagnostics before CLI dispatch and emits one opt-in startup event. The existing `PerfTracker` remains the performance counter owner, but its periodic report is emitted through the `niribar::perf` tracing target instead of `eprintln!`.

**Tech Stack:** Rust 2024, `tracing`, `tracing-subscriber` with `env-filter` and `fmt`, existing `iced` runtime, existing `NIRIBAR_PERF` environment switch.

---

## Accepted Decisions

- Use `tracing` as the single runtime diagnostics pipeline.
- Default diagnostics filter is `niribar=warn`.
- If `NIRIBAR_PERF=1` is set and `RUST_LOG` is unset, default filter becomes `niribar=warn,niribar::perf=info`.
- If `RUST_LOG` is set to a non-empty value, it wins over the default filter.
- Output is human-readable stderr.
- `--version` and successful `--dmenu` continue to reserve stdout for their intentional output.
- Do not log user content by default.
- Do not instrument runtime handlers in this slice beyond the existing perf report.

## File Structure

- Create: `src/diagnostics.rs`
  - Owns environment parsing, default filter selection, tracing subscriber initialization, and startup diagnostics logging.
- Modify: `src/main.rs`
  - Adds `mod diagnostics`, initializes diagnostics before CLI dispatch, and labels CLI mode for the startup event.
- Modify: `src/app.rs`
  - Converts the existing `PerfReport` output from `eprintln!` to `tracing::info!` on target `niribar::perf`.
- Modify: `src/app/perf.rs`
  - Reuses `crate::diagnostics::perf_enabled_from_env()` so perf activation and tracing defaults share one env interpretation.
- Modify: `Cargo.toml`
  - Adds direct diagnostics dependencies.
- Modify: `Cargo.lock`
  - Updated by Cargo after dependency resolution.
- Already completed during design:
  - `docs/adr/0006-use-tracing-for-runtime-diagnostics.md`
  - `config/default.kdl`
  - `config/new_default.kdl`

---

### Task 1: Add tracing dependencies

**Files:**
- Modify: `Cargo.toml`
- Modify: `Cargo.lock`

- [ ] **Step 1: Add direct dependencies**

In `Cargo.toml`, add these lines under `[dependencies]` after `serde_json = "1"`:

```toml
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter", "fmt"] }
```

- [ ] **Step 2: Resolve dependencies**

Run:

```bash
rtk cargo test --no-run
```

Expected: Cargo updates `Cargo.lock`. Compilation may fail later because `src/diagnostics.rs` does not exist yet if later tasks have already been partially applied; at this step on a clean tree it should compile or fail only on dependency download/network access.

- [ ] **Step 3: Commit dependency change**

```bash
rtk git add Cargo.toml Cargo.lock
rtk git commit -m "chore: add tracing diagnostics dependencies"
```

---

### Task 2: Add diagnostics module and env behavior tests

**Files:**
- Create: `src/diagnostics.rs`
- Modify: `src/main.rs`

- [ ] **Step 1: Register the module**

In `src/main.rs`, add the module declaration near the other root modules:

```rust
mod diagnostics;
```

- [ ] **Step 2: Create failing tests**

Create `src/diagnostics.rs` with only these tests first:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn perf_env_accepts_existing_truthy_values() {
        for value in ["1", "true", "yes", "on"] {
            assert!(perf_enabled_from_value(Some(value)), "{value} should enable perf");
        }
    }

    #[test]
    fn perf_env_rejects_missing_or_other_values() {
        for value in [None, Some(""), Some("0"), Some("false"), Some("TRUE")] {
            assert!(!perf_enabled_from_value(value));
        }
    }

    #[test]
    fn default_filter_is_warn_without_perf() {
        let config = filter_config(None, false);

        assert_eq!(config.spec(), "niribar=warn");
        assert_eq!(config.source(), "default");
    }

    #[test]
    fn default_filter_enables_perf_info_when_perf_is_enabled() {
        let config = filter_config(None, true);

        assert_eq!(config.spec(), "niribar=warn,niribar::perf=info");
        assert_eq!(config.source(), "default");
    }

    #[test]
    fn rust_log_overrides_default_filter() {
        let config = filter_config(Some("niribar=debug"), true);

        assert_eq!(config.spec(), "niribar=debug");
        assert_eq!(config.source(), "RUST_LOG");
    }

    #[test]
    fn empty_rust_log_uses_default_filter() {
        let config = filter_config(Some("   "), true);

        assert_eq!(config.spec(), "niribar=warn,niribar::perf=info");
        assert_eq!(config.source(), "default");
    }
}
```

- [ ] **Step 3: Run tests to verify they fail**

Run:

```bash
rtk cargo test diagnostics
```

Expected: FAIL to compile with unresolved functions/types such as `perf_enabled_from_value` and `filter_config`.

- [ ] **Step 4: Implement diagnostics module**

Replace `src/diagnostics.rs` with:

```rust
use tracing_subscriber::EnvFilter;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DiagnosticsConfig {
    pub(crate) perf_enabled: bool,
    pub(crate) filter_source: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FilterConfig {
    spec: String,
    source: &'static str,
}

impl FilterConfig {
    pub(crate) fn spec(&self) -> &str {
        &self.spec
    }

    pub(crate) fn source(&self) -> &'static str {
        self.source
    }
}

pub(crate) fn perf_enabled_from_value(value: Option<&str>) -> bool {
    matches!(value.map(str::trim), Some("1" | "true" | "yes" | "on"))
}

pub(crate) fn perf_enabled_from_env() -> bool {
    let value = std::env::var("NIRIBAR_PERF").ok();
    perf_enabled_from_value(value.as_deref())
}

pub(crate) fn default_filter_spec(perf_enabled: bool) -> &'static str {
    if perf_enabled {
        "niribar=warn,niribar::perf=info"
    } else {
        "niribar=warn"
    }
}

pub(crate) fn filter_config(rust_log: Option<&str>, perf_enabled: bool) -> FilterConfig {
    match rust_log.map(str::trim).filter(|value| !value.is_empty()) {
        Some(spec) => FilterConfig {
            spec: spec.to_string(),
            source: "RUST_LOG",
        },
        None => FilterConfig {
            spec: default_filter_spec(perf_enabled).to_string(),
            source: "default",
        },
    }
}

pub(crate) fn init() -> DiagnosticsConfig {
    let perf_enabled = perf_enabled_from_env();
    let rust_log = std::env::var("RUST_LOG").ok();
    let filter_config = filter_config(rust_log.as_deref(), perf_enabled);
    let filter = EnvFilter::try_new(filter_config.spec())
        .unwrap_or_else(|_| EnvFilter::new(default_filter_spec(perf_enabled)));

    let _ = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .try_init();

    DiagnosticsConfig {
        perf_enabled,
        filter_source: filter_config.source(),
    }
}

pub(crate) fn log_startup(mode: &'static str, config: &DiagnosticsConfig) {
    tracing::debug!(
        target: "niribar::diagnostics",
        version = env!("CARGO_PKG_VERSION"),
        mode,
        perf_enabled = config.perf_enabled,
        filter_source = config.filter_source,
        "niribar diagnostics initialized"
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn perf_env_accepts_existing_truthy_values() {
        for value in ["1", "true", "yes", "on"] {
            assert!(perf_enabled_from_value(Some(value)), "{value} should enable perf");
        }
    }

    #[test]
    fn perf_env_rejects_missing_or_other_values() {
        for value in [None, Some(""), Some("0"), Some("false"), Some("TRUE")] {
            assert!(!perf_enabled_from_value(value));
        }
    }

    #[test]
    fn default_filter_is_warn_without_perf() {
        let config = filter_config(None, false);

        assert_eq!(config.spec(), "niribar=warn");
        assert_eq!(config.source(), "default");
    }

    #[test]
    fn default_filter_enables_perf_info_when_perf_is_enabled() {
        let config = filter_config(None, true);

        assert_eq!(config.spec(), "niribar=warn,niribar::perf=info");
        assert_eq!(config.source(), "default");
    }

    #[test]
    fn rust_log_overrides_default_filter() {
        let config = filter_config(Some("niribar=debug"), true);

        assert_eq!(config.spec(), "niribar=debug");
        assert_eq!(config.source(), "RUST_LOG");
    }

    #[test]
    fn empty_rust_log_uses_default_filter() {
        let config = filter_config(Some("   "), true);

        assert_eq!(config.spec(), "niribar=warn,niribar::perf=info");
        assert_eq!(config.source(), "default");
    }
}
```

- [ ] **Step 5: Run diagnostics tests**

Run:

```bash
rtk cargo test diagnostics
```

Expected: PASS for all diagnostics tests.

- [ ] **Step 6: Commit diagnostics module**

```bash
rtk git add src/main.rs src/diagnostics.rs
rtk git commit -m "feat: initialize runtime diagnostics"
```

---

### Task 3: Wire diagnostics into process startup

**Files:**
- Modify: `src/main.rs`

- [ ] **Step 1: Add failing CLI mode label tests**

In the existing `#[cfg(test)] mod tests` in `src/main.rs`, add:

```rust
    #[test]
    fn cli_mode_label_identifies_cli_modes() {
        assert_eq!(super::cli_mode_label(&crate::cli::Command::Run), "run");
        assert_eq!(
            super::cli_mode_label(&crate::cli::Command::PrintVersion),
            "version"
        );
        assert_eq!(
            super::cli_mode_label(&crate::cli::Command::IpcCommand("toggle-panel".into())),
            "ipc"
        );
        assert_eq!(
            super::cli_mode_label(&crate::cli::Command::Dmenu(
                crate::cli::DmenuOptions::default()
            )),
            "dmenu"
        );
        assert_eq!(
            super::cli_mode_label(&crate::cli::Command::UsageError("bad flag".into())),
            "usage-error"
        );
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run:

```bash
rtk cargo test cli_mode_label_identifies_cli_modes
```

Expected: FAIL to compile because `cli_mode_label` does not exist.

- [ ] **Step 3: Add CLI mode helper and initialize diagnostics**

In `src/main.rs`, add this helper near `fn main()`:

```rust
fn cli_mode_label(command: &cli::Command) -> &'static str {
    match command {
        cli::Command::Run => "run",
        cli::Command::PrintVersion => "version",
        cli::Command::IpcCommand(_) => "ipc",
        cli::Command::Dmenu(_) => "dmenu",
        cli::Command::UsageError(_) => "usage-error",
    }
}
```

Then replace `fn main()` with:

```rust
fn main() {
    let diagnostics = diagnostics::init();
    let command = cli::parse();
    diagnostics::log_startup(cli_mode_label(&command), &diagnostics);

    match command {
        cli::Command::PrintVersion => println!("{}", env!("CARGO_PKG_VERSION")),
        cli::Command::IpcCommand(cmd) => send_ipc_command(&cmd),
        cli::Command::Dmenu(options) => std::process::exit(run_dmenu_cli(options)),
        cli::Command::UsageError(message) => {
            eprintln!("niribar: {message}");
            std::process::exit(DMENU_USAGE_EXIT);
        }
        cli::Command::Run => {
            if let Err(e) = try_main() {
                match e {
                    AppError::Config(config_error) => {
                        eprintln!("{:?}", miette::Report::new(config_error));
                    }
                    AppError::Other(error) => eprintln!("error: {error}"),
                }
                std::process::exit(1);
            }
        }
    }
}
```

- [ ] **Step 4: Run startup tests**

Run:

```bash
rtk cargo test cli_mode_label_identifies_cli_modes
rtk cargo test binary_entry_point_compiles
```

Expected: PASS.

- [ ] **Step 5: Commit startup wiring**

```bash
rtk git add src/main.rs
rtk git commit -m "feat: wire diagnostics into startup"
```

---

### Task 4: Route existing perf summaries through tracing

**Files:**
- Modify: `src/app.rs`
- Modify: `src/app/perf.rs`

- [ ] **Step 1: Add test for shared perf env interpretation**

In `src/app/perf.rs`, inside the existing tests module, add:

```rust
    #[test]
    fn perf_env_values_are_defined_by_diagnostics_module() {
        assert!(crate::diagnostics::perf_enabled_from_value(Some("1")));
        assert!(crate::diagnostics::perf_enabled_from_value(Some("true")));
        assert!(!crate::diagnostics::perf_enabled_from_value(Some("0")));
    }
```

- [ ] **Step 2: Run test before implementation**

Run:

```bash
rtk cargo test perf_env_values_are_defined_by_diagnostics_module
```

Expected: PASS if Task 2 is complete. This test pins that `app::perf` relies on the root diagnostics env vocabulary.

- [ ] **Step 3: Reuse diagnostics env helper**

In `src/app/perf.rs`, replace `PerfTracker::from_env()` with:

```rust
    pub(super) fn from_env() -> Option<Self> {
        crate::diagnostics::perf_enabled_from_env().then(Self::default)
    }
```

- [ ] **Step 4: Convert perf report output**

In `src/app.rs`, replace the current `Message::PerfReport` branch:

```rust
            Message::PerfReport => {
                if let Some(perf) = &self.perf {
                    eprintln!(
                        "{}",
                        crate::app::perf::format_snapshot(&perf.snapshot_and_reset())
                    );
                }
                Task::none()
            }
```

with:

```rust
            Message::PerfReport => {
                if let Some(perf) = &self.perf {
                    tracing::info!(
                        target: "niribar::perf",
                        "{}",
                        crate::app::perf::format_snapshot(&perf.snapshot_and_reset())
                    );
                }
                Task::none()
            }
```

- [ ] **Step 5: Run focused perf tests**

Run:

```bash
rtk cargo test perf_report_resets_counters
rtk cargo test tracker_records_counts_and_resets_snapshot
rtk cargo test format_snapshot_includes_counter_groups
```

Expected: PASS for all three tests.

- [ ] **Step 6: Commit perf tracing conversion**

```bash
rtk git add src/app.rs src/app/perf.rs
rtk git commit -m "feat: emit perf summaries through tracing"
```

---

### Task 5: Final verification

**Files:**
- Read: `docs/adr/0006-use-tracing-for-runtime-diagnostics.md`
- Read: `config/default.kdl`
- Read: `config/new_default.kdl`

- [ ] **Step 1: Verify ADR and docs are present**

Run:

```bash
rtk rg -n "Use tracing for runtime diagnostics|RUST_LOG=niribar=debug|NIRIBAR_PERF=1" docs/adr/0006-use-tracing-for-runtime-diagnostics.md config/default.kdl config/new_default.kdl
```

Expected: output includes the ADR title and both environment variable examples in both default config files.

- [ ] **Step 2: Run full test suite**

Run:

```bash
rtk cargo test
```

Expected: all tests pass.

- [ ] **Step 3: Manual stdout guard for version mode**

Run:

```bash
rtk proxy env RUST_LOG=niribar::diagnostics=debug cargo run -- --version
```

Expected: stdout contains only `0.1.0`. Any diagnostics output must be on stderr.

- [ ] **Step 4: Manual perf compatibility check**

Run Niribar in a Wayland/Niri session with:

```bash
rtk proxy env NIRIBAR_PERF=1 cargo run
```

Expected: periodic human-readable performance summaries appear through tracing without setting `RUST_LOG`.

- [ ] **Step 5: Commit final verification fixes**

If any verification step required small fixes:

```bash
rtk git add Cargo.toml Cargo.lock src/diagnostics.rs src/main.rs src/app.rs src/app/perf.rs docs/adr/0006-use-tracing-for-runtime-diagnostics.md config/default.kdl config/new_default.kdl
rtk git commit -m "test: verify diagnostics initialization"
```

---

## Self-Review

**Spec coverage:**
- `tracing` initialization: Task 2 and Task 3.
- Default `RUST_LOG` behavior: Task 2 tests and implementation.
- `NIRIBAR_PERF=1` compatibility: Task 2 default filter test, Task 4 env reuse, Task 5 manual check.
- Human-readable stderr: Task 2 subscriber setup.
- Startup diagnostics line: Task 2 `log_startup`, Task 3 startup call.
- Perf report conversion: Task 4.
- Minimal docs and ADR: already completed, verified in Task 5.
- No broad runtime instrumentation: no task adds handler logs beyond existing perf report.

**Placeholder scan:** No task contains unresolved placeholder markers. Manual verification requires a Wayland/Niri session because the app itself requires compositor runtime for `cargo run` without `--version`.

**Type consistency:**
- `DiagnosticsConfig`, `FilterConfig`, `filter_config`, `perf_enabled_from_value`, and `perf_enabled_from_env` are defined in Task 2 before use in later tasks.
- `cli_mode_label` is defined in Task 3 before the startup call depends on it.
- The perf tracing target is consistently `niribar::perf`, matching the default filter in Task 2.
