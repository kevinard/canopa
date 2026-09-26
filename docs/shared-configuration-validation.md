# Shared configuration validation

Local phase 3 validation on 2026-09-26, Rust 1.95.0, Linux x86-64, against the
changes following workspace commit `348a37c`. This is debug-build correctness
validation, not a release-performance measurement or a hosted CI result.

Builds reused `CARGO_TARGET_DIR=niribar/target` with `CARGO_INCREMENTAL=0`.
Final executable tests used `CARGO_BUILD_JOBS=1`; the two-job attempt ended when
the system killed the Niribar test compiler with SIGKILL. An earlier build was
superseded by review edits while compiling and mixed old dependency artifacts
with new source. The final single-job run used stable source and passed.

| Check | Result |
| --- | --- |
| Workspace all-target typecheck | Passed |
| Workspace all-target Clippy, warnings denied | Passed |
| Formatting, protocol ownership, vendor provenance, contrast, updater checks | Passed |
| Workspace tests, private D-Bus session | 1,638 passed; zero failed or ignored, across 41 test binaries |
| Product configuration fixture | Passed: validation, namespace isolation, safe errors, explicit rich syntax diagnostics, comment-preserving migration, private output permissions, no-clobber installation |
| Fork library tests | Four passed; layershellev library target compiled with no unit tests |
| Dependency inventory | 547 release packages; notices collected successfully |
| Combined package using full-feature debug binaries | Passed: four binaries execute, common example validates in both owners with matching fingerprints, Wi-Fi provider handshake/close succeeds |
| Full desktop binary build after core validation | Passed |
| Core dependency graph | GUI and audio adapters excluded |
| Core all-target check and tests | Passed; 344 tests, including the compile-fail doctest; zero failed or ignored |

Source-admission tests cover defaults, common/legacy precedence, explicit formats,
missing namespaces, invalid envelopes, byte offsets into the original file,
fingerprints, the exact 1 MiB boundary, overflow, invalid UTF-8, directories,
FIFOs, and regular-file symlinks (including no fallback for dangling shared links).

Notify's configuration tests preserve preparation serialization through cancellation,
last-good settings and source metadata after failures, existing library/CLI legacy
semantics, user rules, namespace isolation, and explicit versus discovered reload
selection. A running discovered daemon adopts a newly created common file on reload.
Niribar checks run without a desktop and reject conflicting CLI modes before IPC.

Both implementation review axes were rechecked after fixes:

- **Standards:** no unresolved material findings. Configuration-check mode cannot
  dispatch IPC; explicit local syntax diagnostics retain the immutable source;
  migration output distinguishes creating a candidate from applying it.
- **Spec:** no unresolved material findings. The installed shell source is visible
  under default logging, and both owners retain safe normal diagnostics.

Logs and artifacts:

- `/tmp/canopa-config-final-checks.log`: strict checks and the two-job compiler exit.
- `/tmp/canopa-config-full-tests-final.log`: successful full workspace suite.
- `/tmp/canopa-config-core-checks.log`: core validation.
- `/tmp/canopa-config-vendor-tests.log`: shared fork tests.
- `/tmp/canopa-config-desktop-build.log`: full-feature binaries restored after core checks.
- `/tmp/canopa-config-package.log`: packaging and smoke test.
- `/tmp/canopa-config-package/canopa-x86_64-unknown-linux-gnu.tar.gz`: smoke-tested
  bundle containing the full-feature debug executables.

No live desktop services or user configuration files were changed. This phase does
not change UI geometry or dependency patches; graphical acceptance from the preceding
slice is recorded in [integration validation](integration-validation.md). Shell
settings remain restart-required, Notify reloads remain process-local, and shared
appearance is a subsequent phase. No release or fork was published.
