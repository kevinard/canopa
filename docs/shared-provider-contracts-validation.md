# Shared provider contract validation

Validation against baseline `36da593`, scoped by
[ADR 0006](adr/0006-shared-provider-contracts.md). Builds use the workspace's locked
dependencies, `CARGO_TARGET_DIR=niribar/target`, `CARGO_INCREMENTAL=0`, and one Cargo
job. No upstream dependency version or fork changes are involved.

The shared crate's targeted run passed 19 tests: the existing v1/v4 contract tests
and five compatibility fixtures. Those fixtures assert the original v1 frame bytes,
secret-input and selectable-list field names, rejection of host-only content,
unchanged handling of a provider-supplied search field, byte-preserving v4 forwarding,
multiple frames and EOF, exact-limit admission, bounded overflow consumption,
truncation, unsupported versions, malformed JSON, invalid actions, and parity
between synchronous and asynchronous v4 readers.

The workspace type-check passes. The protocol's default normal/build dependency
graph contains Serde/JSON dependencies only; Tokio I/O is opt-in. The existing
Wi-Fi process fixture now imports its wire types directly from the shared crate.
The application tests continue through the existing host/provider seams; moved
wire tests are counted once rather than retained as duplicate shell tests.

| Review axis | Remaining findings | Resolved finding |
| --- | --- | --- |
| Standards | 0 | Root license inventory now records MIT and both source attributions |
| Spec | 0 | The new dependency graph check permits fetching on fresh checkouts while keeping the lockfile fixed |

Core validation (`bash scripts/check.sh --core`) passed all 365 tests, with no
failures or ignored tests. This includes 15 protocol tests without its optional
Tokio feature. Core dependency checks continue to exclude GUI/audio adapters.
The initial sandboxed attempt was stopped by its Unix-socket fixtures being denied;
the successful rerun used a private D-Bus session outside the sandbox.

Full validation (`bash scripts/check.sh`) passed all 1,671 workspace tests across
50 executables, with no failures or ignored tests. The count excludes the one
nested accessibility rerun and four fork tests, which also passed. This includes
Wi-Fi ordered credential edits, progress/close cancellation and backend cleanup;
Center malformed frames, paging, stale actions, image bounds, local echo, retained
widget identity, focus and accessibility behavior. Formatting, strict Clippy,
42 contrast pairs, seven updater checks, service ordering, configuration fixtures,
fork provenance and the 551-dependency attribution inventory also passed.

Packaging and the release smoke fixture passed. All four bundled binaries execute;
the bundled Wi-Fi provider completes the existing v1 handshake and close. The
archive includes the shared protocol's MIT notice crediting both source projects.

The nested-desktop fixture passed with real Sway/Niri compositors, dark appearance
and scale 1.25. It verified built-in Center opening, keyboard focus, scrolling to
and invoking the fourth action, banner action delivery, image/icon pixels, DND,
appearance reload, shell restart, daemon loss/recovery and retained attention.
The captured desktop image was also inspected. This is automated compositor and
accessibility-fixture coverage; a full Orca assessment and physical Wi-Fi hardware
acceptance remain outside this extraction. Validation did not change the live user
configuration or services.

Local evidence:

- `/tmp/canopa-protocol-unit.log`
- `/tmp/canopa-protocol-core.log`
- `/tmp/canopa-protocol-full.log`
- `/tmp/canopa-protocol-package.log`
- `/tmp/canopa-protocol-package-smoke.log`
- `/tmp/canopa-protocol-package/canopa-x86_64-unknown-linux-gnu.tar.gz`
- `/tmp/canopa-protocol-desktop.log`
- `/tmp/canopa-protocol-desktop/`
