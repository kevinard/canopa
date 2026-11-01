# Canopa Pinentry

Add an on-demand native Wayland GnuPG pinentry using shared Canopa presentation.
Own the bounded Assuan stdin/stdout protocol independently of GnuPG's GPL frontend
implementation; no new daemon, credential verifier or external password cache.

Acceptance: passphrase/repeated passphrase, confirmation/message dialogs; caller
labels, error/context details, cancellation/timeout/peer disconnect; percent-escaped
UTF-8 and bounded protocol frames; secret buffers erased; direct and forwarded
CLI, configuration owner, packaging/updater/documentation. Verify with unit tests,
a private Wayland peer and a private real GnuPG agent without touching host keys or
GnuPG configuration. Preserve existing work and known Files recovery failure.

## Implementation

`canopa-pinentry` owns a bounded Rust Assuan server and an ordinary Wayland dialog.
It connects to Wayland only while prompting, draws on changes, and watches pipe
hangup without consuming queued commands or polling. Toolkit key repetition is
avoided so cloned cleartext key events are not retained across requests. Explicit
queue/window teardown precedes each Assuan response. A fixed Canopa heading stays
visible when the compositor supplies no titlebar. Context pages preserve all
wrapped lines, including descriptions containing many explicit newlines.

Shared Secret gains an initially selected fixed capacity (default remains 1024;
pinentry uses 8193 bytes for 2048 Unicode scalar values). Shared SessionView can
hide the response field for confirmations/messages while retaining controls.
No new native dependencies, services or host configuration changes are introduced.
The executable/config owner/forwarding CLI, updater/release inventory, native
acceptance manifest and setup documentation are integrated.

## Validation

- Pinentry: 7 unit tests plus the configuration/headless-preview integration test
  pass. Covers repeat mismatch/retry, cancellation erasure, long Unicode bounds,
  full multiline details, default confirmation focus, escaping/framing, reset and
  malformed/unsupported requests.
- Shared configuration: 12 intake/theme tests; shared UI: 4 tests; polkit: 3 unit
  tests plus configuration test; CLI: 3 tests pass (31 focused checks in total).
- Native pinentry, private Wayland and real GnuPG 2.4.9 symmetric encrypt/decrypt:
  final branded-dialog run passes all 15 checks, including repeat status/mismatch,
  cancellation, confirmation/message, details/input erasure, timeouts, EOF with
  queued pipe input and no submitted secrets in diagnostic logs. No host keys,
  GnuPG configuration or services were changed. Reports/screenshots are in
  `desktop-final/` and `desktop-final-heading/`.
- Existing native polkit fixture passes all 22 checks (`polkit-regression/`).
- Strict workspace Clippy passes in `workspace-check-final.log`.
- Updater: 20 tests; desktop runner: 8; check-script contracts: 8; offline session
  unit graph, vendor regression tests, eight-owner configuration tooling and settings-portal activation/
  reads/reload pass. The first settings-portal attempt could not bind its private
  D-Bus in the sandbox; its authorized rerun passed.
- Dependency inventory: 606 packages; release documentation staging: 46 files and
  62 resolving offline links. Affected Bash/Python syntax and `git diff --check`
  pass. A release build/archive smoke run was not performed.
- Initial workspace gate was interrupted with exit 143 after its updater tests,
  before Rust validation (`workspace-check.log`). The fresh process-session run
  recorded in `workspace-check-final.log` completed with exit 101: the same
  pre-existing `canopa-files --test recovery_catalog` failure seen in the avatar
  and polkit work: `pages_remain_loaded_and_saved_identity_survives_active_undo_expiry`
  panics at `crates/canopa-files/tests/recovery_catalog.rs:20` with "Recovery catalog
  did not settle". No Files code was changed for pinentry. The gate reached strict
  workspace Clippy/inventory success and earlier Rust suites, then stopped in
  Files, so later workspace suites are not claimed. Relevant feature/shared/CLI
  tests were run separately as above. The standalone configuration/portal/tooling
  checks cover the applicable steps after the failed suite.

Native acceptance checks actual private client/compositor behavior and a real
GnuPG round trip. It does not establish distribution packaging, machine login,
remote/terminal fallback or accessibility acceptance. Shared text rendering has no
AT-SPI tree/complex-script shaping; owned secret storage is erased but not mlocked.
Unsupported optional protocol features are listed in `docs/pinentry.md`.
