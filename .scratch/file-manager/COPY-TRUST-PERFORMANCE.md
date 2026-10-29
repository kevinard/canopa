# Trust upstream Copy: output fingerprint and optimized measurements

Started 2026-10-25; validation continued on 2026-10-26.
Baseline: `699c793`, clean tree on `main`.
Decision: [ADR0155](../../docs/files/adr/0155-trust-upstream-copy.md).
The full Files goal remains open.

Historical checkpoint: [ADR0156](../../docs/files/adr/0156-fast-content-fingerprints.md)
subsequently removes history compatibility and switches content fingerprints to
XXH3-128. See [current evidence](COPY-XXH3-PERFORMANCE.md).

## Scope and behavior

The user explicitly authorized simplifying Copy to trust uutils, hash the output
once for Undo, retain Recovery/destructive safeguards and rerun optimized
benchmarks. This supersedes the original migration plan's requirement to compare
source/output contents independently. No SHA-1 or other algorithm migration was
requested; SHA-256 and schema60 fingerprints remain compatible with old histories.

`prepared_copy` still owns one lifecycle for Copy, Merge/retry, cross-filesystem
Move, Recovery export, saved-container copying and copied Undo returns. After
successful uutils transfer and its existing byte-count/output-length check,
`copied_output_digest` hashes the held actual output through EOF with positioned
reads. There is no source content reread, second buffer, equality loop or new
verification policy parameter. Source metadata/namespace checks and an output
snapshot around hashing remain. Mode 0000, writer offset, read errors and
cancellation retain their existing behavior. Final metadata restoration and sync
still precede publication and completion.

Ordinary Copy now accepts a mapped source change without a metadata transition;
its Undo fingerprint describes actual output. A same-inode stage edit before
hashing is likewise included in the actual-output fingerprint. Those behaviors
are deliberate, not bugs hidden by changed tests. Concurrent modification does
not have atomic-snapshot semantics.

The expected source fingerprint carries that output digest with the original
source metadata. It is not independently measured during preparation. Existing
destructive paths still read source and output before source removal/disposal;
a mismatch retains the source. Copied-leaf restart, saved-container cleanup and
copied Undo returns keep their existing content/identity checks. No changes to
their production code, durable ownership, replacement retention, fsync barriers,
publication, legacy history parsing or upstream code are part of this change.

## Optimized measurements

The actual root release profile is unchanged: optimization 3, thin LTO, one codegen
unit, panic abort and stripped symbols, without native-CPU flags. This headless
build uses the production operation engine, not the GUI:

```sh
python3 /tmp/canopa-uutils-isolated.py cargo build -p canopa-files \
  --no-default-features --release --example copy_measure --locked --offline -j1
```

Build passed in **4m44s**, log `/tmp/canopa-copy-trust-build.log`. Freeze the
result before another build: `target/copy-opt-evidence/output-only`, SHA-256
`d9cdd60dbde09c1d5b39d08091d19a9f16090593c51ba86ecad0a40ce898f629`.
The unchanged previous executables are `paired` (699c793 behavior, hash
`4a134abf0edb862ae017aee85d458347b9d481d9472d5110568c26c1e7f22754`) and
`pre-migration` (bc433f3 reference, hash
`17d5cdfbbd2f4427722d8d22951825eb40a0a1364db336c2db18ffa642bc924e`). Their
source/profile provenance is in COPY-HASH-PERFORMANCE.md. No old executable was
rebuilt for this comparison, and no source was swapped into the working tree.

After one warmup for each variant/filesystem, alternate/rotate five rounds of all
three variants on tmpfs and three rounds of `paired`/`output-only` on local ext4.
All runs use the unchanged committed `examples/copy_measure.rs`: 128 MiB complete
Copy, 100 small files, first-byte cancellation and separate mover/source hash/
output hash/sync probes. Each removes its own fresh private dataset. The launcher
uses the private XDG driver and then `env TMPDIR=<base> <frozen-executable>`, so the
driver's default TMPDIR does not accidentally put ext4 observations on tmpfs.
Bases are `/tmp` and `target/copy-opt-evidence/private-files`. No Cargo process or
other tests run during measurement. No cache dropping, mount changes or real
user-folder operations are used.

Medians:

| Filesystem | Variant | 128 MiB Copy (s) | 100 small files (s) | Cancel (ms) |
| --- | --- | ---: | ---: | ---: |
| tmpfs | pre-migration reference | 1.324260 | 0.423767 | 8.784 |
| tmpfs | paired comparison + one SHA | 1.437007 | 0.441810 | 8.352 |
| tmpfs | trusted Copy + output SHA | 1.481350 | 0.509013 | 7.178 |
| local ext4 | paired comparison + one SHA | 21.000861 | 18.570993 | 251.065 |
| local ext4 | trusted Copy + output SHA | 21.567066 | 18.986973 | 218.201 |

**This run demonstrates no throughput improvement.** The new large-Copy median is
3.1% higher on tmpfs and 2.7% higher on ext4; it is 11.9% above the fresh tmpfs
pre-migration reference. Large-Copy ranges overlap: tmpfs old 1.418112–1.528701 s,
new 1.361557–1.498085 s; ext4 old 20.801083–22.003489 s, new
21.363560–22.061590 s. These small samples do not establish performance equivalence
or prove a new regression's cause. In particular, report the slower small-file
median rather than claiming that all performance is unchanged; its tmpfs ranges
also overlap (old 0.433920–0.534532 s, new 0.433292–0.519506 s).

The simplification removes one full logical source reread, a 64 KiB buffer and the
comparison loop from production preparation. It removes no SHA compression pass
relative to 699c793. Standalone tmpfs output-hash medians remain about 1.07–1.08 s
per 128 MiB, versus mover medians about 0.10 s, on this i5-6200U without SHA
instructions. Ext4 standalone sync medians are 19.101973 s before and 20.110698 s
after. They are separate probes, not a causal subtraction from complete Copy.

Whole-workload user CPU medians are 3.794356→3.753964 s on tmpfs and
5.909932→5.655975 s on ext4; these include the two unchanged standalone hash probes
and the Python launcher, so they are not isolated engine CPU measurements. All
variants emit 2,049 progress events. First-byte cancellation never reaches the
receipt hash, so its timings cannot be credited to this change.

Raw receipts: `/tmp/canopa-copy-trust-bench.jsonl`, summary with sample counts and
all min/median/max values `/tmp/canopa-copy-trust-summary.json`, launcher
`/tmp/canopa-copy-trust-bench.py`, console log `/tmp/canopa-copy-trust-bench.log`.
These are warm-cache local observations. The ext4 volume reports rotational=0
and had 2,003,648,512 free bytes of about 175 GiB at measurement start. Cold storage,
larger-than-memory files, rotational media and an optimized desktop build are not
measured. No speedup on those workloads is claimed.

## Verification

The normal final entry point ran in the private XDG/runtime driver:
`python3 /tmp/canopa-uutils-isolated.py bash scripts/check.sh`, with a private
D-Bus session for its Rust tests. `/tmp/canopa-copy-trust-workspace.log` records:

- Vendor/protocol/contrast checks, native-fixture inventory and eight desktop-runner
  tests pass. This is fixture listing, not new native GUI acceptance.
- All 19 updater tests pass in 149.530 s; session ordering and formatting pass.
- Workspace all-target Clippy with `-D warnings` passes in 2m08s; dependency
  attribution remains 599 packages.
- The whole workspace all-target test build passes in 25m07s.
- The full default Files library passes: **370 passed, 34 ignored**, 208.61 s,
  including all four output-receipt tests, source/placement boundary checks and
  the existing native-completion/owner-kill Recovery checks.
- Before stopping, the Files portion has **663 passed, 1 failed, 63 ignored**
  across 40 standard parent summaries; child-helper summaries are not counted
  twice. Earlier non-Files Rust targets add 24 passes across nine summaries. The
  custom device-runtime harness also reports its GIO/guard cases separately.
- `recovery_catalog` stops the script with **3 passed, 1 failed**, 11.15 s, at
  `pages_remain_loaded_and_saved_identity_survives_active_undo_expiry`. The final
  workspace entry point is therefore **not green**. Later workspace targets and
  the script's later Settings/configuration/vendor tests were not reached.

The same fresh catalog executable was rerun once with `RUST_BACKTRACE=1`, in a
new private runtime: **the same initial `answer_replace` call at test line 358,
through line 44 and wait line 20**, before the Reader starts or the questioned
Copy executes. It fails in 10.57 s. This matches the preserved pre-migration
baseline and the previous hashing checkpoint; no new cause or fix is claimed.
Receipt: `/tmp/canopa-copy-trust-catalog.log`. Parent counts and target results:
`/tmp/canopa-copy-trust-workspace-summary.json`.

After the catalog stop, run the affected complete targets directly from the fresh
workspace-built desktop-feature executables, without another Cargo build. The
runner checks executable freshness against changed Rust sources, records each
SHA-256/Cargo fingerprint, and holds the debug output lock throughout to prevent
rebuilding a live crash helper. It continues after a failed target.

| Target | Passed | Failed | Ignored |
| --- | ---: | ---: | ---: |
| recovery_directories | 14 | 0 | 1 |
| recovery_ownership | 7 | 0 | 0 |
| request_source_retention | 29 | 0 | 1 |
| transfer_source_remainders | 2 | 0 | 0 |
| transfer_stage_recovery | 2 | 0 | 1 |
| transfers | 32 | 0 | 1 |
| undo_changed_copy | 9 | 0 | 1 |
| undo_cross_filesystem | 20 | 0 | 1 |
| undo_destinations | 4 | 1 | 1 |
| undo_saved_stage_move | 24 | 0 | 1 |
| uutils_copy | 6 | 0 | 0 |
| Total | **149** | **1** | **8** |

The only affected-target failure is the inherited, unchanged 500-item/20-second
Undo timing gate: **25.952629478 s**. Its fixture is retained at
`/tmp/canopa-destination-undo-crash-oVp1J1`. This is a debug batch-Undo result,
separate from the optimized Copy benchmarks; no timing fix is claimed. The normal
workspace entry point and this follow-up therefore remain red on known failures,
with no new failure in the exercised paths. The eight earlier unavailable-Undo
baseline failures were not rerun or claimed resolved in this follow-up.

Receipts: `/tmp/canopa-copy-trust-affected.log`,
`/tmp/canopa-copy-trust-affected-summary.json`, exact executable identities in
`/tmp/canopa-copy-trust-built-targets.json`, runner
`/tmp/canopa-copy-trust-run-built.py`.

Initial focused public test run:
`cargo test -p canopa-files --no-default-features --test uutils_copy --locked
--offline -j1` in the private XDG/runtime driver: **6 passed**, 1.73 seconds after
a 1m12s build (`/tmp/canopa-copy-trust-contract.log`). Two tests prove actual-output
Undo across reopen when source/output differ. Two cross-filesystem Move tests
prove mismatches retain the source and leave the completed Copy undoable. The
metadata/raw-link and concurrent-job tests also pass. The inherited headless-only
`RecoveryPage.observation_paths` dead-code warning remains.

The fresh default desktop-linked executable also passes its real private
`--internal-check-copy` lifecycle: Copy, Move, in-file cancellation, completed-prefix
history, reopen and Undo. Its SHA-256 is
`7c8fa0b76624b3d29032289100e68e158e5813e7b0f72dd2630485be09638f99`.
The smoke freezes a private copy, confirms the built app is newer than changed
production sources and differs from the baseline, and checks that the original
executable's identity/size/mtime stay stable during execution. Receipt and log:
`/tmp/canopa-copy-trust-app-smoke.{json,log}`. This is a default debug application
smoke, not a new packaged release, optimized GUI or native desktop acceptance.

Before default build outputs replaced the prior app, preserve it at
`target/copy-opt-evidence/app-699c793.tar.gz`, archive SHA-256
`4e1d1fcbac9e8f64352951bfdd4d11800aacad935d0c4123edc37ce9514c986e`;
its binary hash is `e4b327b78bf8fd36ccdb16167cb45be31bb696d22519097b2d47efa64759691f`.
The preservation receipt is `/tmp/canopa-copy-trust-protected.json`. No broad cache,
old failure fixture, upstream fork or private schema61 experiment was removed or
changed. Successful private datasets and the smoke's private executable are cleaned.

Remaining limits: the catalog and batch-Undo failures above, previously recorded
generic Undo/Recovery gaps, sparse/reflink/ACL/xattr/hard-link limitations, cold
storage performance and wider native/release acceptance. This completes the
requested Copy-policy simplification and optimized rerun, **not the full Files goal**.

## Independent review

### Standards

No documented violations or compelling baseline smells found. The output digest
helper has one responsibility; preparation retains lifecycle coordination. The
expected-source meaning is explicit. ADR0155 records the authorized policy and
supersedes the old comparison requirement. No dependencies or vendor code change.

### Spec

No findings against the latest instruction. Output hashing, snapshots, descriptor
ownership, cancellation, metadata and sync remain. Destructive consumers and
interrupted-stage disposal retain their separate content/ownership checks. Tests
distinguish accepted Copy behavior from Move source retention. Measurements and
broader validation were still pending at this read-only review.

Summary: Standards 0 findings; Spec 0 findings. Neither reviewer ran Cargo or
tests concurrently with the parent's work.

Both reviewers subsequently checked the completed evidence and supersession
updates: no remaining findings or documentation/code inconsistencies. The pending
evidence is closed; no full Files acceptance is implied.
