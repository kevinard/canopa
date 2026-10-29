# XXH3-128 content fingerprints: evidence and optimized measurements

Date: 2026-10-26. Baseline: `be20ce9`, initially clean tree on `main`.
Decision: [ADR0156](../../docs/files/adr/0156-fast-content-fingerprints.md).
The full Files goal remains open.

## Scope

The user removed the development-history compatibility constraint that retained
SHA-256 in the previous Copy simplification. All content fingerprints now use
XXH3-128: actual copied output, path/opened-file readers and raw symlink targets.
Undo, Recovery and source removal use that same representation. The successful
uutils transfer is still trusted; Copy does not reread the source for equality.

The existing full reads, snapshots, descriptor/namespace checks, cancellation,
metadata restoration, fsync barriers, stage ownership, publication, retained
replacements and destructive-action sequencing are unchanged. No sampled or cached
checksum replaces full content reads. The selected checksum is non-cryptographic,
as appropriate to the user's stated use case. Unrelated small SHA-256 journal and
object-binding checks are untouched.

The new required fingerprint field is `xxh3_128`, containing 16 canonical big-endian
bytes. The earlier 32-byte SHA `digest` field is not accepted. There is no
conversion, fallback or old-algorithm dispatch. SQL schema60 tables are unchanged;
the required field identifies the incompatible receipt format. Old file-content
histories may fail to load or Undo. No actual development history, saved payload,
user file or device is reset or deleted by this task.

Use upstream `xxhash-rust` 0.8.19, `xxh3` feature only, BSL-1.0. Cargo fetched and
verified the published package; its locked archive checksum is
`550a2b930b62486a393c52d5c3b84bff264b28aa437ed64694d31e93b1757af7`.
Lock comparison confirms one new package and no changed existing versions or
checksums (`/tmp/canopa-copy-xxh-lock.json`). The normal release graph grows from
599 to 600 packages; dependency attribution was regenerated. No fork is changed.
The packaging notice collector also preserved the dependency's original LICENSE
at `target/copy-opt-evidence/xxh-notices/xxhash-rust-0.8.19/LICENSE`, SHA-256
`c9bff75738922193e67fa726fa225535870d2aa1059f91452c411736284ad566`.
This checks the new dependency's notice collection, not a newly packaged release.

Primary references: [xxHash](https://xxhash.com/),
[Rust implementation](https://github.com/DoumanAsh/xxhash-rust),
[0.8.19 streaming API](https://docs.rs/xxhash-rust/0.8.19/xxhash_rust/xxh3/struct.Xxh3.html).
The crate selects SSE2 at compile time on x86_64 by default; no AVX2/native flags
are added. Performance claims below use our actual build, not upstream throughput
claims.

## Focused verification

The initial six public Copy/Move tests pass with XXH3. A seventh case replaces an
actual Copy's fingerprint with an old SHA receipt and proves refusal without
changing source, copied output or the stored record. Its first assertion expected
`InvalidData`; the existing SQL decoder wraps that refusal as `Other`. The test
now asserts the specific unknown-old-field refusal and unchanged data, rather
than changing production error wrapping. Final headless run: **7 passed**, 0.74 s,
`/tmp/canopa-copy-xxh-contract-final.log`.

Two new unit cases cover canonical empty/`abc` XXH3-128 values and strict refusal
of old/mixed/wrong-width receipts. Canonical values were independently checked
against the system `libxxhash.so.0` C implementation via its `XXH3_128bits` API.
Existing EOF, cancellation, snapshot-edit and mode-0000/offset tests remain.

One malformed saved-stage journal fixture now uses a valid current content
fingerprint, so it still tests its invalid movement claim rather than failing at
an obsolete hash field.

## Optimized measurement

The production engine uses the unchanged root release profile (optimization 3,
thin LTO, one codegen unit, panic abort, stripped symbols), without native-CPU
flags. Build passed in **4m24s**:

```sh
python3 /tmp/canopa-uutils-isolated.py cargo build -p canopa-files \
  --no-default-features --release --example copy_measure --locked --offline -j1
```

Freeze and hash both executables before further builds:

- SHA baseline `target/copy-opt-evidence/output-only`, SHA-256
  `d9cdd60dbde09c1d5b39d08091d19a9f16090593c51ba86ecad0a40ce898f629`,
  the preserved be20ce9 measurement executable.
- XXH3 candidate `target/copy-opt-evidence/xxh3-128`, SHA-256
  `d586d011a05cce89cb3de976a651c05fc6fc7a4c6521aabb42cc50730b8f2c05`.

The complete-Copy workload and timer boundaries are unchanged: 128 MiB, 100 small
files and first-byte cancellation. The example's two separate hash probes now
measure the active XXH3 algorithm and identify it in JSON; the frozen baseline
probes SHA-256. Thus complete Copy timings are comparable, while whole-workload
CPU/wall time also includes that deliberate probe change and must not be described
as isolated engine CPU savings.

After one warmup per variant/filesystem, alternate five rounds on tmpfs and three
on repository ext4. No Cargo builds or other tests run during measurements. The
launcher uses private XDG/runtime directories, fresh data per run, and explicit
child `TMPDIR` overrides. Source/output are freshly written and cache-warm. No
cache dropping, mount changes, target-CPU flags or user-folder operations occur.

Medians:

| Filesystem | Content fingerprint | 128 MiB Copy (s) | 100 small files (s) | Cancel (ms) | Separate output hash (s) |
| --- | --- | ---: | ---: | ---: | ---: |
| tmpfs | SHA-256 | 1.327461 | 0.420086 | 7.789 | 1.042772 |
| tmpfs | XXH3-128 | 0.327797 | 0.414164 | 6.868 | 0.048074 |
| local ext4 | SHA-256 | 19.096465 | 17.296151 | 208.548 | 1.104451 |
| local ext4 | XXH3-128 | 18.128186 | 16.864582 | 242.705 | 0.048788 |

Complete Copy time falls **75.3% on tmpfs** (4.05 times the throughput), and
**5.1% on this ext4 volume**. The separate tmpfs hash probe is about 21.7 times
faster. These are measured observations, not a subtraction of independent probes
from engine time. Tmpfs Copy ranges are disjoint: SHA 1.281565–1.352031 s, XXH3
0.312094–0.343564 s. Ext4 ranges overlap: SHA 17.456332–19.115672 s, XXH3
16.214996–18.648650 s. Independent ext4 sync medians remain large and variable:
16.971100 s before, 16.075772 s after; do not attribute all disk timing variation
to hashing or promise a universal disk speedup. All samples emit 2,049 progress
events. Cancellation stops before hashing, so its differences cannot be credited
to the new algorithm.

The ext4 data base is `target/copy-opt-evidence/private-files`; it had
4,511,223,808 free bytes at measurement start. The valid tmpfs observations use
the private `/dev/shm/canopa-xxh-bench-qnjathjc` base. Both variants use that same
tmpfs; do not mix these results with earlier `/tmp` measurements.

Receipts: `/tmp/canopa-copy-xxh-build.log`,
`/tmp/canopa-copy-xxh-bench.{jsonl,log,py}` and
`/tmp/canopa-copy-xxh-summary.json` (all sample counts and min/median/max values).
This is the optimized headless engine on an i5-6200U without SHA instructions,
not an optimized desktop GUI, cold-media, rotational or larger-than-memory test.

## Temporary-storage quota incident

The first baseline warmup on `/tmp` stopped before a result with a quota error.
An isolated repeat captured SQLite's `operation history: disk I/O error` at open;
ordinary sandbox setup also reported `Quota exceeded` for its synthetic `/tmp/.git`
mount target. The volume still reported free space. No throughput sample from
either failed attempt is included. Logs are preserved as
`/tmp/canopa-copy-xxh-bench-quota.{log,jsonl}` and
`/tmp/canopa-copy-xxh-bench-io-error.{log,jsonl}`.

The first warmup left one generated 349 MiB data tree, `/tmp/.tmpxoUJ41`. Its
known layout, fixed-size all-`a` source and small-file payloads were checked, then
every regular file was hashed and archived. Every archived file was reread and
matched against that manifest before removing only this uncompressed generated
tree. Archive: `target/copy-opt-evidence/xxh-quota-fixture.tar.gz`, SHA-256
`8589e84aa4796c6fec96a5919c106ad936dcef67b00c5cfb471f7488b156febb`;
exact paths, device/inode values and per-file hashes:
`target/copy-opt-evidence/xxh-quota-fixture.json`. No other failure fixture or cache
was cleared. The valid run uses a fresh private tmpfs base outside `/tmp` and
completes both variants successfully.

Before later default builds replace the baseline app, preserve it as
`target/copy-opt-evidence/app-be20ce9.tar.gz`, archive SHA-256
`84f88354c7ed156b91a7f205a388f018b4f17b7a07b3b27f28293299719daf18`.
The preserved binary SHA-256 is
`7c8fa0b76624b3d29032289100e68e158e5813e7b0f72dd2630485be09638f99`;
receipt `/tmp/canopa-copy-xxh-protected.json`. Earlier frozen references and the
private schema61 experiment are untouched.

## Final validation

Resumed 2026-10-29 after the session environment restarted. `/tmp` logs and helper
scripts from the earlier run are no longer available, including the unfinished
workspace run; its final runtime results cannot be established. The earlier
completed benchmark observations above were recorded before that restart. The
frozen benchmark executables, protected app archive, quota-fixture archive and
dependency LICENSE remain in `target/copy-opt-evidence/`; their hashes were checked
again and match this report. `target/copy-opt-evidence/resume-integrity.json` also
records the current lock/code hashes and a repeat lockfile audit: one added
package, no changed existing versions or checksums. The detailed original benchmark samples were in
`/tmp` and are no longer available; the table and ranges above are the retained
measurement record.

Repeat workspace validation with fresh isolated user/config/runtime state:

```sh
python3 target/copy-opt-evidence/resume-isolated.py bash scripts/check.sh
```

The new receipt is `target/copy-opt-evidence/resume-workspace.log`, outside `/tmp`.
The fresh run passes the vendor/protocol/contrast checks, desktop-runner tests
(8 cases, 1.776 s), updater tests (19 cases, 146.386 s), session-unit checks,
formatting, workspace all-target Clippy with warnings denied (3m53s), and the
600-package release attribution check. The all-target test build completes in
30m45s. This broad gate builds one executable per integration-test file, alongside
workspace unit/bin/example targets. The private driver also carried
`CARGO_INCREMENTAL=0` from measurement setup into debug validation, unnecessarily
reducing reuse. Future development loops should retain debug incremental builds
and select the Copy/fingerprint tests first, followed by the affected Undo/Recovery
targets; reserve the full workspace gate for final validation.

The workspace run stops in the Files library: **368 passed, 4 failed, 34 ignored**,
235.85 s. Four child crash workers do not reach their requested boundary within
their unchanged 25-second deadline. All four parent cases pass individually using
the exact same built binary: 7.54 s, 4.66 s, 3.07 s and 1.28 s. No code, test limit
or receipt format is changed to obtain those passes. These are the cases:

- `copy_boundary::recovered_stops_retry_their_end_save_before_gaining_continuation_owners`
- `copy_boundary::source_return_disposition_rolls_back_and_refuses_changed_task_proofs`
- `directory_boundary::a_completed_source_directory_removal_restores_only_its_result`
- `directory_boundary::a_finished_directory_leaves_only_later_batch_work_to_continue`

Receipts: `target/copy-opt-evidence/resume-unit-failures.{log,json}`. All eight
failed source/output fixture roots remain in place. Their 50 entries are also
archived and content-verified in
`target/copy-opt-evidence/resume-unit-failure-fixtures.tar.gz`, SHA-256
`bb9e47a374c8894510d47034e79c89dce3d17c3f001efed8b597ce5cfa67b1b1`;
the matching `.json` records original paths and per-file checksums. The full
parallel workspace run is not green.

The completed Cargo artifact list is saved in
`target/copy-opt-evidence/resume-artifacts.{jsonl,log}`. Retrieving it with the same
`cargo test --workspace --all-targets --no-run --message-format=json` settings
takes 3.67 s and performs no new compilation. A runner holds the debug Cargo
output lock while executing the selected binaries and records each executable's
hash, features and unchanged identity. Supplemental default-feature results:

| Target | Passed | Failed | Ignored | Seconds |
| --- | ---: | ---: | ---: | ---: |
| recovery_catalog | 3 | 1 | 0 | 10.99 |
| recovery_directories | 14 | 0 | 1 | 3.10 |
| recovery_ownership | 7 | 0 | 0 | 0.79 |
| request_source_retention | 29 | 0 | 1 | 19.04 |
| transfer_source_remainders | 2 | 0 | 0 | 0.10 |
| transfer_stage_recovery | 2 | 0 | 1 | 1.14 |
| transfers | 32 | 0 | 1 | 4.85 |
| undo_changed_copy | 9 | 0 | 1 | 15.39 |
| undo_cross_filesystem | 20 | 0 | 1 | 169.24 |
| undo_destinations | 4 | 1 | 1 | 51.80 |
| undo_saved_stage_move | 24 | 0 | 1 | 85.05 |
| uutils_copy | 7 | 0 | 0 | 0.77 |

Total: **153 passed, 2 failed, 8 ignored** across 12 complete targets. Receipts:
`target/copy-opt-evidence/resume-affected.{log,json}` and
`resume-affected-summary.json` in the same directory. These checks cover actual
output receipts, source retention, Undo changes, Recovery ownership, metadata and
link policy, and old-receipt refusal. All six fingerprint unit cases also pass
in the workspace run, including both new format/vector cases.

The two integration failures remain open:

- The known catalog timeout repeats at its initial `answer_replace` call,
  `recovery_catalog.rs:358`, before `Reader::start` or copying. A fresh isolated
  backtrace repeat fails at the same call in 10.29 s:
  `target/copy-opt-evidence/resume-catalog.log`.
- The unchanged 20-second 500-item Undo gate takes **23.769327158 s**. Its original
  `/tmp/canopa-destination-undo-crash-rrxuYw` is retained. All 1,507 entries are
  archived and content-verified in
  `target/copy-opt-evidence/resume-undo-timing-fixture.tar.gz`, SHA-256
  `89e13a325cfd796f59044a739b0c35c91cd69bf1950d45e5424c3842f006875f`,
  with a matching JSON manifest. This is debug batch Undo, not optimized Copy.

The newly linked default-feature app passes its private `--internal-check-copy`
smoke in **0.40 s**: Copy, Move, in-file cancellation, completed-prefix history,
reopen and Undo. A private frozen executable is used, and the live executable's
identity/size/mtime are checked unchanged. SHA-256:
`3e86c0175135d5e87d0db9640e070a911f1e69ccc833b77b991be848796394c5`;
receipts `target/copy-opt-evidence/resume-app-smoke.{log,json}`. This is the debug
desktop-linked app, not a packaged release or native GUI acceptance run.

Formatting, Clippy, attributions and diff checks pass. The workspace gate remains
failed at its parallel Files library run; later stages of that script are not
claimed. The four sequential unit repeats, supplemental integrations and app
smoke do not turn it into a clean full-workspace result. The broader Files goal,
inherited history-binding failures, native acceptance and sparse/reflink/ACL/xattr/
hardlink-group limitations remain open as recorded in the migration ledger.

## Independent review

### Standards

No documented violations or compelling baseline smells found. Content hashing
stays separate from the shared lifecycle and destructive coordination. The
required algorithm-specific field makes the incompatible development format
explicit. The fixture and dependency/license updates retain their intended scope.

### Spec

No concrete spec or safety findings. Content readers use XXH3 consistently while
full reads and existing safeguards remain. Unknown old receipts are refused
before destructive consumers receive a typed Change; expiry errors retain the
record. No conversion, reset, fallback or unknown-hash acceptance was found.

Standards: 0 findings; Spec: 0 findings. Both reviews were read-only and ran no
builds, tests or fixture writes. Final validation receipts were pending at review.
Subsequent edits complete this validation report; the reviewed production/test
implementation is unchanged.
