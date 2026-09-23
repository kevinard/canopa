**Niribar project audit — 2026-09-23**

Implementation follow-up: [remediation and verification record](2026-09-23-remediation.md). This audit describes the pre-fix baseline.
Snapshot: `0dbd269` on `main`; working tree was clean when the audit began. Scope: current application source, configuration, local requirements, architecture decisions, tests, vendored integration notes, and CI/release definitions. This is a whole-project audit, not a review of only the latest commit.

**Assessment**

Niribar is a substantial working application with a reasonable core architecture, but it is not yet ready to be presented as a complete, reliably integrated release. The principal weaknesses are unfinished real backends, integration lifecycle defects, and release verification. The UI and state machines are often further along than the adapters that supply their data.

Code quality is acceptable as a foundation for continued development. The explicit Foreground, Panel Runtime, Bar Surface Registry, Module Runtime, and intent/effect boundaries are valuable. Another broad architectural rewrite is not the immediate priority. Complete the backend behavior, add tests at the transport boundaries, restore the quality gate, and make the documented support level accurate.

**Verified health**

| Check | Result |
| --- | --- |
| `cargo fmt --all -- --check` | Pass |
| `cargo test --locked --all-targets` | Pass: 1,155 tests, zero failures and zero ignored tests |
| Native Canopa accessibility test | Passed using its isolated D-Bus/AT-SPI fixture |
| `cargo clippy --locked --all-targets -- -D warnings` | Fail: compilation stopped after 24 diagnostics |
| Focused IPC probe | Confirmed that an active dmenu response blocks a subsequent toggle command |
| Focused command probe | Confirmed that a descendant process survives the command timeout |
| Focused launcher probe | Confirmed that a user `Hidden=true` override fails to hide the matching system entry |

Toolchain: `rustc 1.97.0-nightly (14196dbfa 2026-04-12)` and `cargo 1.97.0-nightly (eb94155a9 2026-04-09)`. CI selects stable rather than this installed toolchain, so the exact Clippy diagnostic count may differ there. Ordinary compiler warnings already include private-interface mismatches and unused production code; this is not solely a new Clippy preference.

The first test attempt failed because the execution sandbox prevented the accessibility fixture from binding its private D-Bus socket. The full suite passed after rerunning with local IPC access. The 1,155 total counts the accessibility test once, excluding its nested fixture invocation. The Wi-Fi binary itself has zero unit tests; its higher-level provider tests use a fake backend, and its process handshake test selects the no-backend path.

The temporary probes reused the production IPC, command, and desktop-entry functions, with minimal unrelated type scaffolding and a private temporary runtime directory. They did not connect to the running bar or change the user's network. Source files were not modified. Full check logs and the probe are in `/tmp/niribar-audit-*` and `/tmp/niribar_audit_probe.rs`; these temporary files are not durable project artifacts.

**Capability inventory**

| Area | Current state |
| --- | --- |
| Bars and configuration | Substantial implementation: named Bars, exact output targets, edges/layers, styles, formatting, actions, diagnostics, and embedded defaults |
| Modules | Clock/calendar, script, button, battery, volume, network status, focused window, workspaces, and tray implementations exist |
| Surfaces and interaction | Wayland output planning/reconciliation, Foreground lifecycle, keyboard navigation, tooltips, Panels, and Menus exist with extensive state tests |
| Selection and launcher | Configured menus, fuzzy search, installed applications, plain/rich dmenu input, and selection responses exist; see correctness gaps below |
| External Panel Providers | Framed v1 and Canopa v4 transports, validation, failure UI, lifecycle handling, bounded local images, and Canopa accessibility exist |
| Wi-Fi control | UI/state machine is implemented; real NetworkManager and iwd adapters are materially incomplete |
| Distribution | Release workflow and systemd unit exist; release contents, onboarding, and reproducibility need work |

**Standards and runtime correctness findings**

Priority meanings: P1 should be addressed before describing the affected feature as reliable; P2 is a concrete correctness or robustness defect to schedule next. Findings are code-confirmed unless explicitly marked as reproduced. They are separate from the completeness findings below.

**S1 — P1: Backend subscriptions do not recover.**

[D-Bus subscription handling](/home/kevinard/projects/niribar/src/dbus/subscription.rs:72) transitions to `Done` after an initial error or a running source failure, then exhausts the stream. [Niri does the same](/home/kevinard/projects/niribar/src/niri/subscription.rs:170). The application retains the same subscription identities without a restart mechanism; D-Bus initialization is also one-shot. Starting before a dependency becomes available, or losing a backend connection, can leave affected modules unavailable until Niribar restarts. This conflicts with the current [module recovery contract](/home/kevinard/projects/niribar/src/modules.rs:221).

Introduce persistent recovery with bounded backoff and cancellation. Verify initial absence, disconnect, and recovery through actual transport fixtures, including restarting the service while a populated module is visible.

**S2 — P1: Tray item changes are not consumed.**

The [tray host event loop](/home/kevinard/projects/niribar/src/dbus/sni/host.rs:294) listens for registrations, unregistrations, and bus-name changes. It does not subscribe to the item's icon/status/tooltip change signals declared in [the proxy](/home/kevinard/projects/niribar/src/dbus/sni/proxy.rs:40). After registration, an application's changed icon, attention state, passive state, or tooltip can remain stale indefinitely.

Maintain per-item signal subscriptions and refresh the relevant properties. Test several updates from the same registered item without re-registering it.

**S3 — P2: Tray registrations are not fully retired.**

[Path-based registration](/home/kevinard/projects/niribar/src/dbus/sni_watcher.rs:26) stores an owner plus object path, such as `:1.42/StatusNotifierItem`. [Owner-loss handling](/home/kevinard/projects/niribar/src/dbus/sni/host.rs:334) removes only a key equal to the bare bus name. The built-in watcher never removes stored registrations or emits its declared unregistration signal. This leaves ghost icons for path registrations; retained well-known registrations can also suppress re-registration notifications after an application restarts.

Track owner and object identity separately and retire all items for a vanished owner in both the watcher and host. Cover path and well-known-name registrations.

**S4 — P2: Open tray menus receive only one live update.**

[The menu subscription](/home/kevinard/projects/niribar/src/dbus/sni/subscription.rs:110) waits for one layout/property change, emits `LayoutChanged`, and transitions to `Watching => None`. The subscription key stays the same while the menu remains open. Further changes to labels, checked states, enabled states, or layout are missed.

Keep both signal streams alive for the lifetime of the Menu. The regression case must send at least two successive changes while it remains open.

**S5 — P2: Niri disconnection leaves stale workspace children.**

[Unavailable handling](/home/kevinard/projects/niribar/src/app/niri.rs:64) changes the `Workspaces` parent's state. Visible dynamic children use `WorkspaceItem`, and [rendering supplies them an output state unconditionally](/home/kevinard/projects/niribar/src/app/render.rs:246). Their previous workspace data and actions survive the disconnect.

Propagate availability through the Dynamic Module Family or reconcile its children to an empty set. Test a disconnect after an initial populated workspace snapshot.

**S6 — P1: A pending dmenu request blocks all subsequent IPC. Reproduced.**

The [single listener task](/home/kevinard/projects/niribar/src/ipc.rs:85) accepts one connection and then awaits the UI's dmenu response inside that accept loop, at line 110. Until the user finishes selection, another dmenu request cannot reach the application's busy-rejection logic and `toggle-panel` cannot be handled. A connected client that never finishes writing also blocks the listener.

The isolated probe observed no toggle event during a 200 ms pending-selection window; the toggle arrived immediately after resolving the first dmenu response. Serve connections independently, with bounded concurrency and an input-read deadline. Preserve the intentional lack of a timeout while a valid request waits for user selection. Add socket-level tests for concurrent dmenu rejection and toggle delivery.

**S7 — P2: IPC size limits are enforced after unbounded reads.**

[The server](/home/kevinard/projects/niribar/src/ipc.rs:89) reads the entire connection into a growing `Vec` before inspecting the protocol header or the 1 MiB limit. [The dmenu CLI](/home/kevinard/projects/niribar/src/main.rs:103) similarly reads all stdin before validating entries and frame size. The limit therefore does not bound memory consumption during input collection.

Decode framed input incrementally, reject an oversized advertised payload before allocating it, and bound legacy command input. Apply an input budget to CLI collection as well. This is a local availability defect, not evidence of a remote attack surface.

**S8 — P2: A second instance can take over the first instance's IPC pathname.**

[Socket cleanup](/home/kevinard/projects/niribar/src/ipc.rs:25) removes any existing socket without checking whether a server is alive. A second running Niribar can unlink the first instance's listening pathname and bind the same path. The older bar remains running but control requests target the new instance.

Distinguish a stale socket from a live instance, using a lock/ownership strategy and a connection check. Report bind/setup failures instead of silently leaving an inert subscription.

**S9 — P2: Command timeout cleanup kills only the immediate process. Reproduced.**

[Async cleanup](/home/kevinard/projects/niribar/src/command.rs:50) and [action timeout cleanup](/home/kevinard/projects/niribar/src/command.rs:222) kill one child process. Explicit shell commands can spawn descendants that continue running after the shell is killed. A probe running `sleep 3 & ...; wait` returned `TimedOut` after 100 ms while the spawned `sleep` still existed.

Use process-group ownership for commands whose entire execution must be bounded, and terminate the group on timeout/cancellation. Apply a deliberate policy to shell-backed providers too. Preserve the separate lifecycle of intentionally launched desktop applications. Add a regression test that checks a descendant's termination rather than only the parent result.

**S10 — P2: Installed-application discovery loses desktop-entry precedence. Reproduced.**

[Application collection](/home/kevinard/projects/niribar/src/desktop_apps.rs:77) filters entries independently without resolving duplicate Desktop Application IDs first. A user override with `Hidden=true` is discarded, then the matching system entry is still included. Ordinary user overrides can produce duplicates. The probe returned one visible system application where the hidden override should suppress it. This conflicts with desktop-entry [ID precedence](https://specifications.freedesktop.org/desktop-entry/latest/file-naming.html) and [Hidden semantics](https://specifications.freedesktop.org/desktop-entry/latest/recognized-keys.html).

Select the first entry per ID in directory precedence order before visibility filtering. Also isolate malformed/unreadable files: currently a single failed file read can propagate through discovery, and [the caller converts that failure to an entirely empty launcher](/home/kevinard/projects/niribar/src/app/panel.rs:1122).

**Spec and completeness findings**

**C1 — P1: NetworkManager Wi-Fi proxy construction is incomplete.**

[Device proxy construction](/home/kevinard/projects/niribar/src/bin/niribar-wifi-provider.rs:247) sets a path but no destination. The device, wireless-device, and access-point [proxy declarations](/home/kevinard/projects/niribar/src/bin/niribar-wifi-provider.rs:441) omit `default_service`. The pinned zbus implementation requires that destination and does not infer it from the interface name. Enumeration therefore fails as soon as a returned device requires constructing one of these proxies.

Set the service explicitly and test the real adapter against a private-bus fake. Higher-level `FakeBackend` tests cannot detect this construction failure. This finding was checked against the locally installed pinned zbus/macro source; it was not reproduced against the user's NetworkManager service.

**C2 — P1: Real Wi-Fi control does not implement the advertised flows.**

[NetworkManager scan/connect](/home/kevinard/projects/niribar/src/bin/niribar-wifi-provider.rs:303) only rereads a snapshot for scan, and ignores every connection request before returning `AuthorizationRequired`. [iwd connection handling](/home/kevinard/projects/niribar/src/bin/niribar-wifi-provider.rs:392) rejects all requests carrying credentials and all hidden-network requests. Its snapshot also labels every discovered network as personal at line 368, causing unknown open networks to enter the unsupported credential flow.

The [current provider requirements](/home/kevinard/projects/niribar/docs/external-panel-providers.md:108) and `.scratch/wifi-panel-provider/issues/02` through `05` require known/open joins, personal credentials, hidden joins, persistence, and both backends. Enterprise/EAP setup is explicitly outside V1 and is not counted as missing here.

Implement the actual D-Bus operations and credential/profile lifecycle. Until then, accurately mark the control feature as partial and avoid presenting unconditional failure as a user authorization problem.

**C3 — P1: The CI definition contains an invalid job condition.**

[The Nix job condition](/home/kevinard/projects/niribar/.github/workflows/ci.yml:52) calls `hashFiles` in job-level `if`. GitHub permits that function in step conditions, but not that job context, according to its [context availability table](https://docs.github.com/en/actions/reference/workflows-and-actions/contexts#context-availability). This is a workflow validation defect rather than a reliable mechanism for disabling an absent Nix package. No GitHub Actions run was executed during this audit.

Move repository-file detection to a step after checkout or a prerequisite job output. Once the workflow is valid, address the locally failing warning-as-error gate. Explicitly provision the AT-SPI/D-Bus fixture dependencies rather than assuming the runner image has them.

**C4 — P2: Release contents omit the default Wi-Fi provider binary.**

[The release upload](/home/kevinard/projects/niribar/.github/workflows/release.yml:47) includes only `niribar`. The default network action resolves `niribar-wifi-provider` beside the executable or through PATH, so a fresh binary-only installation cannot open the provider successfully. Building the companion binary does not distribute it.

Package both executables and installation guidance, with the existing default configuration and systemd unit available as appropriate. Smoke-test the extracted release bundle with a clean PATH. The embedded default configuration does exist, so the missing external config artifact does not itself prevent startup.

**C5 — P2: Wi-Fi progress is returned only after the operation completes.**

[Connection handling](/home/kevinard/projects/niribar/src/wifi_provider.rs:305) awaits the backend before returning a vector containing the connecting and final documents. The [binary event loop](/home/kevinard/projects/niribar/src/bin/niribar-wifi-provider.rs:74) sends those documents only afterward. During the actual operation, progress is absent and input/close processing waits. The loop also has no source of unsolicited backend state changes.

Emit progress before starting the operation and multiplex completion, provider input, and backend updates through an explicit state machine. A fake that deliberately stays pending should prove that the connecting document is already observable. Test duplicate input arriving during that pending operation, rather than inspecting only a completed vector.

**Code-quality improvements**

- Restore the warning-as-error gate. Examples include public fields exposing less-visible types in `app/messages.rs:41` and `modules/focused_window.rs:13`, unused production constructors/old helper methods, and eight additional style/complexity diagnostics on this toolchain. Remove obsolete paths or constrain visibility; avoid broad lint suppression.
- Move installed-application filesystem discovery off the synchronous GUI update path. [The effect adapter](/home/kevinard/projects/niribar/src/app/overlay_adapter.rs:103) currently scans directories during panel preparation. Use a background task with a request generation check; introduce caching only after measuring whether it is needed.
- Keep strengthening existing module boundaries rather than splitting files merely for length. For example, `module_runtime.rs` is about 700 lines of production behavior followed by more than 2,300 lines of tests. Raw file size overstates its production complexity.
- Add transport and process lifecycle tests. Highest-value scenarios are two successive signals, service disappearance/reappearance, simultaneous IPC clients, delayed operations, process descendants, and clean release installation. The current unit tests are useful but miss these interactions.
- Make dependency/toolchain verification reproducible: declare a supported Rust baseline, use locked dependency checks in CI/releases, and document local vendor patches and upgrade checks. AccessKit already has a useful `LOCAL-PATCHES.md`; preserve that level of traceability for other forks.
- Keep the rotation experiment and its vendored renderer copies clearly separate from supported application code. They are outside this audit's production line count and root test coverage.

**Documentation and project-state gaps**

There are 46 implementation issue files under `.scratch`: 35 marked `ready-for-agent`, 11 `ready-for-human`. Many describe behavior already present, including Foreground ownership, selectable sessions, provider infrastructure, and module detail sessions. Unchecked boxes cannot be used as a count of missing features. Reconcile each ticket against implementation and validation evidence; archive superseded expansion-era plans.

[ADR 0011](/home/kevinard/projects/niribar/docs/adr/0011-explicit-canopa-panel-protocol-adapter.md:11) still directs users to `canopa-v3` and says screen-reader integration is absent. Current configuration accepts `canopa-v4`, and the provider guide/native accessibility implementation have advanced accordingly. Supersede or amend the ADR and matching glossary references so following architecture documentation does not yield invalid configuration.

No root README, project-level license declaration/file, Nix flake, published performance comparison, or accessibility assessment artifact was found. The systemd unit and documented embedded defaults are present. Decide whether Nix remains a supported commitment; provide build/run/configuration instructions and state current limitations. Treat absent CPU/RSS/startup benchmarks and broader accessibility checks as missing evidence, not proof that performance or accessibility is poor. Native accessibility exercised here is specific to Canopa panels, not a certification of the whole application.

**Recommended order of work**

1. Restore enforceable verification: fix the workflow, make strict lint checks pass, and establish a reproducible toolchain/dependency policy.
2. Complete the Wi-Fi adapters and asynchronous progress behavior, using tests that call the real adapter interfaces against isolated D-Bus services. Correct the documented support level until this is done.
3. Repair recovery and tray subscriptions, with disconnect/reconnect and repeated-signal regressions.
4. Fix IPC concurrency, input bounds, socket ownership, and command descendant cleanup; retain socket/process-level regressions.
5. Correct launcher precedence/error isolation and move scanning off the GUI path.
6. Validate a clean release bundle, reconcile the tracker/docs, then measure a real compositor session: idle CPU/RSS, startup, hotplug, focus/keyboard behavior, scaling/rotation, and accessibility.

No percentage-complete estimate is justified by the available evidence. The application is feature-rich, but the missing work is concentrated in behavior that matters during ordinary desktop use and failures. Fifteen concrete findings are recorded: ten standards/runtime findings and five spec/completeness findings. The most serious standards/runtime problems are nonrecovering subscriptions, stale tray updates, and serialized IPC; the most serious completeness problems are the real Wi-Fi adapters and invalid CI condition.

**Limits of this audit**

This was a deep, targeted review, not an exhaustive proof of every path. No real Wi-Fi connection was attempted; no live PipeWire/Niri restart, output hotplug, graphical session, release build/install, or performance benchmark was performed. Most integration findings are established by control-flow inspection rather than live-system reproduction. Third-party renderer internals and the rotation prototype were not exhaustively audited. No known-vulnerability database scan or full dependency-license review was performed. The only repository change made by the audit is this report.
