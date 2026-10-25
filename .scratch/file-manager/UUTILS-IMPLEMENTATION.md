# uutils Copy backend implementation and evidence

Date: 2026-10-25. Baseline: `bc433f3` on `main`, clean working tree.
Decision: [Files ADR0153](../../docs/files/adr/0153-uutils-prepared-copy.md).
Scope: one complete prepared Copy path and every current byte-copy consumer.
This does **not** complete the full Files goal or all migration acceptance.

Current policy: [ADR0155](../../docs/files/adr/0155-trust-upstream-copy.md) and
[simplification evidence](COPY-TRUST-PERFORMANCE.md) trust upstream Copy and hash
only its actual output. Recovery and destructive-action checks remain. The
earlier follow-up [optimized Copy hashing investigation](COPY-HASH-PERFORMANCE.md) and
[ADR0154](../../docs/files/adr/0154-verified-copy-hashing.md) replace the two SHA
computations recorded here with full source/output byte comparison and one SHA
of their verified common contents. Both full reads and the durable contract
remained at that historical checkpoint. The measurements and test receipts below
describe the original migration, not the current Copy comparison policy.

## Feasibility and selection

The published, non-yanked uu_cp/uucore 0.12.0 archives were fetched from crates.io,
and their SHA-256 values were checked against Cargo's official sparse-index
records. Both declare Rust 1.88 and MIT; this workspace uses Rust 1.95.

- uucore archive: `f24a5910ebbf2c0baf486a9125705f8e026a8f7741a3ea8cf5eca3c9257c9bdf`.
- uu_cp archive: `eb4c15f0ba298a490b66abac0fefc3a44ff1b8004c38cf140d27c027f1f80784`.
- VCS revision: `dc1efd89948a9ca4c78c3a4b9a6ac891019a8c69`.

`python3 scripts/prove-uutils-copy.py` creates a disposable Cargo proof with its
own committed lockfile and private files under target/. It reproduced the default
uu_cp path API copying `/proc/self/fd/N` as a symlink. That API has no supplied
progress/cancellation callbacks, and its path canonicalization and metadata
semantics do not supply our selected-descriptor contract. It is rejected; no
complete Move is passed to cp/mv.

The selected implementation directly links a narrow uucore extension. Each call
owns its pipe and reuses upstream splice/drain/RawWriter helpers and bounded
io::copy fallback. Callbacks run before I/O and after at most 64 KiB, including
in-file cancellation. There is no shared pipe cache, CLI initialization, terminal
parser, spawned thread/process, status aggregation, or background writer.
The final standalone proof measured 128 MiB / 2,048 byte updates, 92,614 us copying and
153 us cancellation; killing/reaping its process left a 65,536-byte partial with
no writer. Raw filename bytes, replaced/restricted parent, cancellation and output
failure followed by another call passed. The caller's uucore exit status remained
73 across successful/error calls. Its raw-link create/read check is only a
transport probe; **actual symlink copying** passes in the composite `uutils_copy`
public engine test (dangling raw target) and existing transfer tests.

The first minimal-feature build exposed a published-package issue: disabling the
signals feature leaves unconditional nix signal references unresolved. Production
uses default signals plus buf-copy (which enables pipes/fs); none of the signal
initializers are called. Root lock resolution adds 16 package records (one local
uucore plus 15 registry packages), with no existing version changes. The default
Linux release inventory is 599 packages, up from 585. There is one vendor copy,
its original MIT notice, complete upstream/local provenance and one reviewable
patch. Offline provenance and exact archive/patch reconstruction pass.

## Implementation and consumer audit

`operations/prepared_copy.rs` owns the former preparation lifecycle: selected
source, registered owned stage/payload, transfer, metadata, host sync, receipt.
`copy_backend.rs` accepts only opened regular files and fixed baseline metadata
policy; it cannot publish, remove, edit history, or resolve conflicts.
`fingerprint.rs` retains source validation and SHA-256; its handwritten 64 KiB
read/write/hash transfer loop is deleted. SHA-256 now requires two separate full reads, of the still-held source and the
readable held output, requiring equal digests and checking the original source
snapshot around both passes. Output uses positioned descriptor reads and final
mode/timestamps are restored after hashing, including atime. Native success and expected length alone never make a durable receipt.

Audited production consumers all use this same lifecycle:

- Ordinary/recursive Copy, Merge, batch retry/continuation and cross-filesystem
  Move in transfer.rs.
- Recovery/export through normal Transfer; saved-container migration in
  saved_stage.rs and pinned Selection::copy_saved in entry_move.rs.
- Cross-filesystem copied Undo returns in undo.rs.

No remaining application byte-copy loop or silent fallback was found. Remaining
fs::copy calls under operations are test-only history fixture copies; write_all
in Trash creates metadata rather than transferring content.

Descriptor/source/device checks, stage registration, retained replacements,
source removal only after committed publication, logical Undo/history/catalog
ownership and live Retry/Continue/Abandon remain Files responsibilities. Schema60
and its old evidence are unchanged. No uutils journal family or schema61 changes.
Direct linkage has no worker executable, protocol, lookup, startup overhead, or
orphan-child acknowledgement window. Kernel I/O can block; cancellation is
cooperative at bounded transfer boundaries, not a guarantee against stalled
kernel/device calls.

Metadata retains baseline behavior: ordinary 0777 mode and regular-file atime/
mtime, creating-user ownership; symlinks use raw targets. ACLs/xattrs and hard-link
groups are not copied, sparse allocation becomes dense, and reflink is not used.
Tests demonstrate the sparse, hard-link and xattr limits. ACL omission follows
the unchanged metadata policy; ACL-specific filesystem acceptance was not run.
Special-file readable opens remain
refused. These are not claims of cp -a or the private cp sparse/reflink engines.

## Measurements

Single debug observations on private temporary files; not optimized throughput
acceptance or a reliable speedup comparison. Existing SHA-256 semantics remain.
The baseline measure executable linked the exact pre-migration Cargo-built rlib
and its fingerprint-matched tempfile dependency; it is preserved under
`target/uutils-preserved/`. Both observations use the same engine workload.

| Observation | Before | After |
| --- | ---: | ---: |
| 128 MiB complete engine Copy (including hash/journal/sync) | 14.441064 s | 21.502788 s |
| 100 small files, one recursive Copy | 2.597790 s | 1.554718 s |
| Cancel at first in-file byte update, including owned cleanup | 63.857 ms | 34.337 ms |
| Progress events | 2,049 | 2,049 |

Separate final 128 MiB component samples: mover 86.524 ms, source SHA-256
10.582650 s, output SHA-256 11.486383 s, fsync 2 us. Hashing dominates the debug
measurement; the complete large-file Copy regresses by about 49%. This is a known
cost of the verified receipt, not a throughput improvement claim.
The discarded one-source-pass prototype did not prove the
staged digest and is not a final performance result. Samples are independent,
not subtractions from the engine run; tmpfs fsync costs differ from disk.
The runnable proof on the repository filesystem also demonstrated dense sparse
output: 8 MiB allocated for an all-hole 8 MiB input. Reflink is not attempted.
There is no per-file worker launch. Native resident startup/idle and optimized
release throughput were not measured and remain open.

## Verification

Initial verification, before the final receipt correction:

- Six source boundary unit tests pass: opened leaf/parent replacement, raw link
  identity, special-type refusal, final checkpoint replacement, replaced output
  payload/stage/parent.
- Complete `operation_transfers`: 29 passed / one ignored parent-invoked helper;
  the helper passes separately within its parent. Includes shared Jobs Copy,
  file/folder conflict choices, Merge, cancellation, observed progress, exact
  stopped results, retry/save, retained sources, forced cross-filesystem Move
  and shared Undo.
- New `uutils_copy`: two passed, composite regular metadata/raw dangling links/
  empty and sparse files/hard-link and xattr policy, plus concurrent independent
  jobs with byte validation and Undo.
- Formatting, shell/Python syntax, root vendor provenance, selected published
  archive checksum/inventory/exact patch, dependency attribution generation pass.

The new completion-cut test initially assumed partial bytes remained at their
original stage pathname. A deterministic private SIGKILL run failed that assertion
(`/tmp/canopa-stage-task-wfaKEa`, retained). Journal and byte inspection showed the
existing disposal claim had moved the full 128 KiB into its separately owned
holding namespace, with no publication and the completed prefix intact. The test
now follows the public Recovery catalog instead of equating a path with ownership;
no production recovery change was made.

The first `scripts/check.sh --core` attempt reached the no-default all-target
check after the Shell core tests passed, then exposed an inherited test wiring
error: device_coordination imports futures-util even though that dependency was
optional in this profile. futures-util is now also a dev-dependency; no production
feature or existing locked version changed. The core entry point was rerun.
Workspace all-target Clippy with warnings denied passed (6m55s); the final script
also rechecks the corrected test source.

The corrected new native-completion crash parent passes both cuts (`transferred`
and `completed`) through real OperationJobs and public Recovery. It verifies full
retained bytes across two reopenings, no premature destination publication,
replacement-source preservation, and an Undo entry for the completed prefix.

The next core stop was another inherited test expectation: the schema22 upgrade
fixture expected 57 even though production already uses 60. That assertion now
expects the unchanged schema60; its complete target and all later integration
targets run with --no-fail-fast. Production schema code was not edited.

Actual pre-migration compatibility also passes: the preserved baseline benchmark
executable (SHA-256 `affa267c9114fd46eb0a3f3ea5d53fa7ab0b5080afcdf143ab254c5779b28e69`)
was SIGKILLed after writing a registered 65,536-byte partial in each of two private
runtimes. Both databases were actually schema60. The current library reopened
each original database twice without rewriting identities. With its original
source present, cleanup completed without publication or invented Undo; with
its selected source replaced, Recovery retained/exposed the old partial and
preserved the unrelated replacement. The driver reaped the writer before reopen.
The two successful large-data fixtures were cleaned after verification; the
baseline executable remains under target/uutils-preserved/. This is a real old
executable handoff, not merely setting user_version on a new database.

The unchanged 500-item Undo gate fails again at **24.998704253 seconds** against
20 seconds, in undo_destinations. Its expanded fixture is
retained at `/tmp/canopa-destination-undo-crash-bqjl27`; no timing assertion or
durability was relaxed. The remaining targets completed without fail-fast.

The complete core Files target set (library plus 68 integration targets, merging
the repaired request_execution target with the continued run) totals **877 pass,
9 fail, 72 ignored helper definitions**. Library alone is 285 pass/33 ignored.
The nine failures are confined to three targets:

- undo_destinations: four pass/one fail/one ignored; the unchanged 20-second gate above.
- undo_unavailable_moves: five pass/seven fail/one ignored; saved-only canonical
  history binding refusals, including the corresponding child crash expectation.
- undo_unavailable_saved: eight pass/one fail/one ignored; source-only Replace
  return work differs from its original history step.

To classify the latter eight failures, both unchanged public test files were
compiled directly against the **pre-migration** canopa_files rlib and its exact
Cargo-fingerprint-matched dependencies, writing private executables under
`target/uutils-preserved/`. The old library produces the identical failure names,
errors and counts: 5/7/1 and 8/1/1 respectively. No canonical/history proof check was
weakened to make these inherited cases pass. Logs are
`/tmp/canopa-uutils-baseline-unavailable-{moves,saved}.log`; the current complete
remaining-target run is `/tmp/canopa-uutils-core-rest.log` and the first core log
is `/tmp/canopa-uutils-core-schema-stale.log`. Shell's core tests also passed;
Notify's final core test step was not reached by that entry point.

The final receipt review found a real gap in that initial source-only hash:
an mmap writer can change bytes again without a new size/mode/mtime/ctime change.
A Python mapped writer reproduced this through the public engine, producing a
completed Copy with a receipt for the wrong bytes. A second public regression
edited the same staged inode before its receipt. Both fail against the original
uutils adapter (two retained `canopa-copy-receipt-*` fixtures;
`/tmp/canopa-uutils-receipt-red.log`). The final implementation hashes actual held
output too, refuses digest mismatch, preserves staged data in Recovery, restores
metadata after reads, and uses an RDWR output/pread so mode 0000 does not require a
permission-sensitive reopen. This adds a second SHA-256 pass, deliberately.
A dedicated readable-FD/mode-zero regression also guards that contract.

The 877/9/72 complete core run above precedes this final verification addition;
final focused/default/package results below distinguish the final source.


## Review and remaining work

### Standards

The independent review found no actionable documented-standard violation or
compelling smell. Its follow-up review of the receipt correction also found no
open lifecycle or standards finding.

### Spec

The initial review found two evidence gaps: component measurements and symlink
evidence attribution. Both are addressed above. The receipt follow-up required
checking actual output in addition to the held source; both checks and the
mode-zero descriptor regression are implemented. No open finding remains within
this scope. Its final read-only review confirms the correction and evidence
distinctions. These reviews do not establish full product acceptance.

The unchanged 500-item/20-second Undo gate remains open (historical schema60
measurements 29.84 s / private 29.50 s); it is not relaxed. Generic incoming-Move
Undo, Trash/Restore restart, legacy disposition/cleanup, broader backend
separation, native/device/accessibility acceptance and complete optimized release
acceptance remain in the wider Files goal. Protected current failure fixtures and
private schema61 sources were not changed. The old target/debug desktop executable
listed by the handoff was absent before this migration. A new final-source
executable is built at that path; no protected old executable was overwritten.

Final-source verification:

- Final default-feature library: **367 pass/34 ignored**, 234.53 s, including the
  new mode-zero FD digest and both native completion cuts. The source contains
  the dual source/output comparison.
- Final no-default public uutils_copy: **four pass**, including the real mapped
  write with unchanged timestamps, same-inode stage edits, metadata/atime/raw
  links/sparse/hard-link/xattr policy and independent concurrent copies.
- Full workspace entry point: vendor/panel/contrast/fixture list, 8 desktop-runner
  tests, 19 updater tests, session-unit ordering, format/Clippy/attribution passed.
  Its test run later stopped at a 10-second Recovery catalog settling timeout (3 pass/1 fail
  in that target). A private GVfs activation also left a disconnected fixture
  mount during test-environment teardown; it was gone when inspected and the
  private environment was cleaned. Subsequent isolation disables GVfs/FUSE and
  system service discovery. No user mount was changed.
- Fifteen final default public targets completed: **245 pass/one fail/12 ignored**,
  including four new uutils tests, Copy/Move/retry, stage Recovery, source
  retention, history upgrade, cross-filesystem Undo and saved-stage Move Undo.
  The one failure is the same catalog timeout. A backtrace locates it in
  answer_replace, before Reader::start or byte copying. The exact pre-migration
  HEAD source was independently compiled with matching default dependencies and
  reproduces the same 10-second timeout at that initial Replace question. This is an
  inherited failure; its cause remains open. Logs:
  `/tmp/canopa-uutils-final-public.log`,
  `/tmp/canopa-uutils-catalog-single.log`,
  `/tmp/canopa-uutils-baseline-catalog.log`.

The full workspace attempt's normal library was built before the final receipt
correction; its library-test target and all final focused/public results above
include the fix. The entry point stopped at the catalog failure, so its remaining
workspace test targets and downstream settings/configuration/vendor tests did not
run in that invocation. Final static, Notify and packaged-binary results follow.

Final workspace all-target Clippy with warnings denied passed (2m03s), after
correcting a test-only octal-notation lint. Formatting, shell/Python syntax,
vendor integrity, dependency attribution (599 release dependencies) and
`git diff --check` pass. The final standalone proof also passes with the global
exit-status assertion. Logs: `/tmp/canopa-uutils-clippy-final.log` and
`/tmp/canopa-uutils-proof-final.log`.

`cargo build --workspace --bins --locked --offline -j1` passed on final source
(4m39s). The new Files binary includes the linked backend and display-free private
Copy/Move/cancel/reopen/Undo check. Log: `/tmp/canopa-uutils-build-final.log`.

The separate final `bash scripts/test-notify.sh --no-default-features` passed:
**384 tests, zero failures**, across 36 targets including doctests, on a private
bus and isolated configuration/data. This completes the Notify core step that the
earlier Files failure prevented the core entry point from reaching. Log:
`/tmp/canopa-uutils-notify-final.log`.

After that core run, the default workspace binaries were restored with the same
locked offline build (5.11s). Both `scripts/package-release.sh` and
`scripts/smoke-release.sh` passed against the actual debug bundle. Both invoke the
packaged Files executable's private Copy/Move/in-file cancel/reopen/Undo check;
the smoke also executes all eight binaries, checks configuration, 24 documentation
files/40 offline links, assets and provider close behavior. The bundled uucore MIT
notice is present. Packaged Files is byte-identical to the final built binary:
SHA-256 `26fc7825839f574146d2c22b819fe2883c7c35c658987771ff368b0c0688ce4a`.
The debug archive is
`target/uutils-release-debug/canopa-x86_64-unknown-linux-gnu.tar.gz`, SHA-256
`16e163f6b035b197614e7019b2bc9bb9ebdf4bbce49cf32aa802cd0a087db7ed`.
Logs: `/tmp/canopa-uutils-{package,smoke}-final.log`; exact receipt:
`/tmp/canopa-uutils-package-receipt.json`. This is packaging validation with debug
binaries, not an optimized release or hosted distribution acceptance claim.

The downstream configuration validation/migration/privacy/no-clobber check passes.
The Settings portal activation/frontend ReadOne/ReadAll/reload check passes on a
private bus with private config/data. Its first run hid all system data directories
and the frontend failed to start; restoring read-only `/usr/local/share:/usr/share`
made the same check pass without code changes. Logs:
`/tmp/canopa-uutils-configuration-final.log`,
`/tmp/canopa-uutils-settings-final.log` (isolation failure),
`/tmp/canopa-uutils-settings-with-system-data.log` (pass).

The full workspace suite is not green: the inherited failures above remain, and
the remaining unselected default integration targets and `scripts/test-vendor.py`
were not rerun after its stop. Native/physical-device/ACL acceptance, resident
startup/idle and optimized throughput remain unmeasured. Under storage pressure,
only 41 completed test ELF outputs from this migration's own run were reclaimed
with Cargo locks held; paths/hashes are recorded in
`/tmp/canopa-uutils-elf-reclamation.json`. Broad caches, protected executables,
failure fixtures and the schema61 experiment were preserved.

Review findings: Standards 0; Spec 2 initial evidence gaps, both resolved; no open
finding in either axis within this scope. The full Files goal remains open.
