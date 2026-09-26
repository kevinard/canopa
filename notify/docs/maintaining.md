# Maintaining Canopa Notify

## Build profiles and ownership

The default Cargo features are `gui` and `audio`. `gui` includes Iced, Wayland,
image preparation and native AT-SPI accessibility. `audio` includes Rodio and its
device/decoder dependencies. The public daemon, lifecycle, storage, body parser,
control and D-Bus interfaces compile without either adapter:

```sh
cargo check -p canopa-notify --all-targets --no-default-features --locked
bash scripts/test.sh --no-default-features
cargo build -p canopa-notify --no-default-features --features gui --locked
```

A core build runs `daemon --headless` and the control/provider commands. It rejects
`--service` with a feature-specific error. A GUI build without audio renders
normally and does not advertise sound. Test presenters remain available in core
builds, so lifecycle and delivery-policy tests do not require a display or device.

| Owner | Responsibility and verification boundary |
| --- | --- |
| `lib.rs`, `runtime.rs` | Small `Daemon` entry point, ordered mutations, revision guards, committed effects and adapter coordination. Public daemon/control tests. |
| `lifecycle.rs`, `domain.rs` | Sender lifecycle, retained attention, residency, transience and action meaning. Lifecycle/history tests. |
| `sqlite_persisted_state.rs`, `migrations/` | Forward migrations, atomic writes, keyset reads and admission/retention. Real temporary SQLite fixtures. |
| `intake.rs`, `notification_link.rs`, `input_limits.rs` | Bounded markup, safe typed targets and input validation. Receipt and malformed-input tests. |
| `notification_server.rs`, `notification_server/` | Exact freedesktop wire interface, live sender-ID registry and ordered publication. Private-bus executable tests. |
| `startup.rs` | Private paths, ownership, legacy import and failure before recovery. Competing-process and migration tests. |
| `control.rs`, `control_adapters/` | Typed controls, bounded sockets, CLI, independent provider pagination and opaque interactions. Real socket/provider tests. |
| `presentation.rs` | Desktop-independent banner projection and presenter contract. Public presenter fixtures. |
| `presentation/renderer.rs` and children | Actual Iced widgets, revision-owned input, surface/display lifetimes, bounded assets and AT-SPI. Widget, native bus and compositor evidence. |
| `sound/playback.rs`, `sound/playback/theme.rs` | Bounded playback and sound-theme resolution. Real offline mixer fixtures. |
| `link_opening/portal.rs`, `activation/wayland.rs` | Desktop opening and best-effort activation context. Private portal mocks and graphical acceptance. |
| `canopa-panel-protocol` | Generic versioned presentation contract; no notification-domain interpretation in the host. Protocol validation and Niribar round trips. |

Keep slow file/device/portal work outside the mutation coordinator. Preserve the
distinction between ordered sender events and replaceable UI projections. A test
at a model boundary does not establish rendering, native accessibility or physical
input behavior; link those separate fixtures in the compliance matrix.

## Vendored source

The root Canopa workspace owns all dependency forks. See
[shared fork maintenance](../../docs/vendor-maintenance.md) for current versions,
provenance, retained patches and upgrade checks. The panel protocol is the single
root `crates/canopa-panel-protocol` source used by both applications.

Run the root `bash scripts/check.sh` and `bash scripts/check.sh --core` entry points.
For Notify-only Cargo commands select `-p canopa-notify`; `--workspace` now also
selects the shell. Root `target/` contains the normal build outputs.

## Evidence discipline

Use `Cargo.lock`, a private session bus, and temporary XDG paths. Run graphical
acceptance in a disposable compositor; never replace a developer's notification
service as a test fixture. Record versions, build revision, workload, machine,
failures and cleanup. The [compliance matrix](freedesktop-notification-compliance.md)
and [completion tracker](../.scratch/freedesktop-completion-review/IMPLEMENTATION.md)
are the release record. An unavailable graphical job is an unmet gate, not a
passing test. Keep native Notification portal v2 separate from the classic bridge.

## Lint and parser checks

CI denies warnings in owned workspace targets with
`cargo clippy --workspace --all-targets --locked -- -D warnings`. Intentional
exceptions have narrow attributes with reasons: public owned lifecycle effects,
private adapter/row construction, and shared integration-test helpers. Do not add
blanket production allowances to hide a new warning. Vendored libraries retain
upstream policy and their documented local patch inventory.

`intake::tests::fuzz_smoke` runs deterministic malformed-markup mutations and
nesting/segment/typed-URI round trips as part of both core and full suites. It is
bounded smoke coverage, not a claim of exhaustive fuzzing or a substitute for
regressions for reported inputs.

## Repeatable desktop check

`scripts/test-desktop.py` requires a compatible Niribar binary, Niri, Sway, grim,
software Mesa, Python GI with Gio/AT-SPI, the AT-SPI launcher/registry, a C compiler,
`pkg-config` and `wayland-scanner`. It builds its small private pointer helper,
starts a nonactivating private bus and fresh XDG directories, then exercises both
actual adapters and sender callbacks. It records screenshots, logs, compositor
versions and results, and terminates only its own process groups.

```sh
python3 scripts/test-desktop.py --notify target/debug/canopa-notify \
  --niribar /path/to/niribar/target/debug/niribar --artifacts /tmp/notify-desktop-new
```

The manually dispatched `Desktop acceptance` workflow runs the same check on a
runner labeled `canopa-desktop` with a supplied compatible host checkout. A runner
must be provisioned before this job can run; the local passing fixture is recorded
separately from hosted CI. This smoke test does not run Orca; follow the separate
[screen-reader procedure](orca-isolated-fixture-research.md) for that acceptance.

Use the [hosted CI setup guide](hosted-ci.md) to provision and register the runner.
`scripts/check-desktop-runner.py --niribar-root /path/to/niribar` checks prerequisites
without touching the live desktop. Registration and hosted results remain open
until a GitHub repository and dedicated machine exist.

## Storage interruption and configuration load checks

`bash scripts/test.sh --test storage_failures --test legacy_import` exercises real
SQLite busy errors, SIGKILL during an uncommitted migration, and SIGXFSZ during a
legacy snapshot write. The last fixture needs `prlimit` from util-linux. These
tests establish process-interruption recovery, not filesystem or power-loss
certification. Reserved `.history-import-*` snapshots and sidecars are reclaimed
under the destination ownership lock on the next default-path startup.

Build the configuration suite with `bash scripts/test.sh --test configuration_intake
--no-run`, then pass the executable path printed by Cargo to:

```sh
python3 scripts/stress-configuration.py --test-binary /path/to/configuration_intake-HASH \
  --artifacts /tmp/configuration-load-new --runs 5 --cpu-workers 4
```

Each repetition creates a private session bus and records its test log and Linux
CPU/memory/I/O pressure. SIGTERM and Ctrl-C terminate its worker/test processes
and retain partial results. Do not count a fixture setup failure as a test failure
or success. Complete ordinary validation after compilation finishes; a concurrent
build is an explicit stress scenario, not the release test baseline.

## Release measurements

Run `python3 scripts/measure-runtime.py /path/to/release/canopa-notify` after builds
finish. It uses its own bus and state, warms up 32 transient receipts, then measures
1,000 transient and 1,000 retained serial Notify/Close pairs with 4 KiB bodies.
JSON includes receipt percentiles, RSS/high-water memory, CPU/platform, binary hash
and clean shutdown. Compare equivalent machines and workloads against the
[recorded release baseline](notification-release-verification.md). This workload
is not a burst or graphical-latency benchmark.

[Dependency attributions](dependency-attributions.md) record the resolved Linux
normal/build graph. Regenerate this inventory when the lockfile or features change;
include the appropriate upstream notices with a distributed release.
