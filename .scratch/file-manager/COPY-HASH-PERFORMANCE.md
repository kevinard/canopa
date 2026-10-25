# Optimized Copy hashing investigation

Date: 2026-10-25. Baseline: `72f56b9`, initially clean working tree.
Decision: [ADR0154](../../docs/files/adr/0154-verified-copy-hashing.md).
The full Files goal remains open.

Historical checkpoint: [ADR0155](../../docs/files/adr/0155-trust-upstream-copy.md)
subsequently replaces paired comparison with trusted upstream Copy plus one
output hash, at the user's request. See [current evidence](COPY-TRUST-PERFORMANCE.md).

## Reproduction

Build the real headless operation engine and measurement example with the root
release profile (`opt-level=3`, thin LTO, one codegen unit, panic abort, stripped
symbols), without target-cpu/native flags:

```sh
CARGO_INCREMENTAL=0 TMPDIR=/tmp CARGO_NET_OFFLINE=true \
  cargo build -p canopa-files --no-default-features --release \
  --example copy_measure --locked --offline -j1
target/release/examples/copy_measure
```

This measures production Copy, 100 small files, in-file cancellation, and separate
transfer/source-hash/output-hash/sync components on private files. The GUI is not
part of this optimized build. The machine is an x86_64 i5-6200U with AVX2 and no
SHA instruction flag; SHA-256 uses sha2 0.10.9's default runtime-selected portable
fallback. No dependency, hashing algorithm or CPU requirement is changed.

The baseline build passed in 10m19s. Three initial tmpfs samples gave complete
128 MiB Copy 2.493097–2.658675 s; separate source hashes 1.073586–1.123794 s, output
hashes 1.083447–1.086060 s, mover 108.846–132.547 ms, sync 2 us. The original debug
21.50 s result was not extrapolated into an optimized performance claim.

A private workspace extracted the actual pre-migration Files source at
`bc433f355e6f5afc8d376fd16b06d442ed08cccb`. Its dependency manifest, features,
measurement example, root release profile and patches match the current build;
only production Files source comes from the old commit. Active registry
packages were checked against the root lockfile before building offline. Its
optimized build passed in 4m24s; initial complete Copy samples were
1.275036–1.451385 s. This confirms the optimized regression independently of the
debug measurements. This old executable is a measurement reference, not a
candidate rollback of verification.

Frozen binaries are under `target/copy-opt-evidence/`:

- `dual-digest`, baseline 72f56b9, SHA-256
  `5848345d8ee3dcd371546962b8cbe0138396dd53917bc5ef50936aadb04fa200`.
- `pre-migration`, baseline bc433f3, SHA-256
  `17d5cdfbbd2f4427722d8d22951825eb40a0a1364db336c2db18ffa642bc924e`.

Initial raw observations: `/tmp/canopa-copy-opt-{baseline,old}.jsonl`.
Build logs: `/tmp/canopa-copy-opt-{baseline,old}-build.log`.
The private old workspace and exact revision are indexed by
`/tmp/canopa-copy-old-opt.json`; no production source was replaced for that build.

## Hypotheses and change

Ranked before changing verification:

1. Redundant SHA compression dominates: full byte equality plus one SHA should
   remove about one hash's CPU cost.
2. Additional read I/O dominates: retaining both complete reads would leave most
   of the regression.
3. Journal and identity checks dominate: changing verification would barely move
   complete Copy time.

The candidate reads both complete streams, compares their bytes and EOFs, and
hashes the proven common contents once. Both descriptor snapshots span the full
comparison, and the original caller snapshots/namespace checks remain. Output
reads retain the readable FD and writer offset, including mode 0000. Metadata,
sync, publication, source removal and Undo/Recovery authority are unchanged.
No checksum cache, shortened input, weaker hash, new dependency or silent fallback
is introduced. Both full verification reads remain; cold-device I/O savings are
not claimed.

## Final optimized measurements

The candidate optimized build passed in 4m31s. Its frozen `paired` executable has
SHA-256 `4a134abf0edb862ae017aee85d458347b9d481d9472d5110568c26c1e7f22754`.
After one warmup per executable, variants were alternated/rotated for five rounds
on tmpfs and three on repository ext4. No Cargo builds or other agent tests ran
during measurements. Files were freshly written and cache-warm; each run removes
its own private dataset. The repository volume reports rotational=0. These are
local observations, not cold-cache or general device throughput acceptance. The
repository volume had about 1.1–1.4 GiB free of 175 GiB during those measurements;
reclaiming old test ELFs afterward changed that condition.

Medians (same 128 MiB/100-file workload and release profile for all variants):

| Filesystem | Variant | 128 MiB Copy (s) | 100 small files (s) | Cancel (ms) |
| --- | --- | ---: | ---: | ---: |
| tmpfs | pre-migration | 1.302878 | 0.495077 | 8.477 |
| tmpfs | dual SHA | 2.512335 | 0.448667 | 9.113 |
| tmpfs | paired comparison + SHA | 1.477073 | 0.450631 | 7.654 |
| repository ext4 | pre-migration | 21.452704 | 18.514895 | 299.828 |
| repository ext4 | dual SHA | 20.535040 | 18.704568 | 269.658 |
| repository ext4 | paired comparison + SHA | 20.551906 | 18.941335 | 329.774 |

The tmpfs Copy median improves **41.2%**, saving 1.035262 s, consistent with removing
one 1.1 s hash. It remains 13.4% above the pre-migration reference, which hashes
during its custom byte-copy loop instead of doing the current post-copy source/
output verification. Small-file measurements are essentially unchanged.
All variants emitted 2,049 progress events. Cancellation does not enter
verification in this probe, so its differing samples are not attributed to the
hashing change.

On repository ext4, there is **no measurable end-to-end Copy speedup**. Independent
128 MiB sync samples have medians 18.545148 s (dual) and 18.544079 s (paired), masking
the CPU saving. No sync, journal commit or publication barrier was removed.
Whole-benchmark user CPU decreases from 4.718329 s to 3.666871 s on tmpfs and from
6.814505 s to 5.638689 s on ext4. Those totals include the two unchanged standalone
hash probes after the engine workload; they are not isolated engine CPU counters.
Component samples are independent observations, not subtractions from Copy time.

Raw alternating observations: `/tmp/canopa-copy-opt-comparison.jsonl`; complete
median/min/max summary: `/tmp/canopa-copy-opt-summary.json`. The timed production
algorithm is unchanged by the later test-only compared-chunk fault hook.

Hypothesis 1 is supported for CPU-bound Copy. Hypothesis 2 does not explain that
tmpfs regression; both reads remain after the fix. Storage durability dominates
the local ext4 observations, so hashing improvements alone do not improve that
wall-clock result. Pairing also interleaves source/output reads at 64 KiB;
cold-cache, larger-than-memory and rotational-storage behavior is unmeasured.

Prevention: measure optimized complete operations and their transfer/hash/sync
components before judging a backend change. Keep receipt semantics as behavioral
tests, so verification can be optimized without treating two computations as the
contract. The existing private verification seam supports those tests; no broader
architecture change was needed for this fix.

## Validation

- Optimized public `uutils_copy`: **four passed**, 0.34 s, after an 8m53s optimized
  test build. This includes mapped-source writes without a new timestamp,
  same-inode stage edits with retained Recovery data, independent concurrent
  copies/Undo, raw links and metadata policy. Log:
  `/tmp/canopa-copy-opt-contract.log`.
- Four focused verification unit tests passed (0.16s, no default features):
  SHA/block/chunk/EOF boundaries and unequal lengths, cancellation before/during
  comparison, source/output edits to already-compared bytes, mode 0000 and
  unchanged output offset. Source/output mutation tests force an mtime change so
  they are deterministic even on coarse clocks. The hook is test-only,
  thread-local, consumed once and cleared by a guard. Log:
  `/tmp/canopa-copy-receipt-tests.log`.
- Normal `bash scripts/check.sh`, in an isolated runtime: vendor integrity,
  panel/contrast/fixture checks, 8 desktop-runner tests, 19 updater tests, session
  ordering, format, workspace all-target Clippy with warnings denied (2m05s), and
  the 599-package attribution check pass. The workspace test build passed in
  24m17s. The default Files library passes **370/zero failures/34 ignored** in
  228.05s, including native-completion crash cuts and all four verification tests.
- The entry point stops at the inherited Recovery catalog settling timeout:
  three pass/one fail in that target. Its pre-migration reproduction and location
  before copying are recorded in UUTILS-IMPLEMENTATION.md. Files parent summaries
  through that stop total 663 pass/one fail/63 ignored (40 standard-summary
  targets, including the library and zero-test binary). The separate device
  runtime harness also passes. Log: `/tmp/canopa-copy-workspace.log`; parsed
  parent summaries: `/tmp/canopa-copy-workspace-summary.json`.
  A fresh single-case backtrace confirms the same initial `answer_replace`
  call (recovery_catalog.rs:44 from:358), before Reader startup, as the preserved
  pre-migration reproduction. Log: `/tmp/canopa-copy-catalog-location.log`.
- Nine affected complete targets after that stop pass **124 tests**, with one
  inherited timing failure and six ignored child-helper definitions. They cover
  Recovery ownership, retained sources, source remainders, stage Recovery,
  transfers, cross-filesystem Undo, saved-stage Move Undo, destination Undo and
  the four public copy regressions. Cross-filesystem Undo passes 20 cases;
  saved-stage Move Undo passes 24. The unchanged 500-item/20-second Undo gate fails
  at **28.052798044 s**, preserving
  `/tmp/canopa-destination-undo-crash-MEl5sU`. No limit was relaxed. Logs and
  per-target counts: `/tmp/canopa-copy-affected-tests.log` and
  `/tmp/canopa-copy-affected-summary.json`.

The continuation executes the fresh, workspace-built test ELFs directly to keep
the exact workspace feature graph. Selection requires desktop Cargo fingerprints
and executable modification times after both changed source files; the manifest
records exact paths, SHA-256 values, fingerprints and exits in
`/tmp/canopa-copy-built-targets.json`. A target-directory lock prevents helpers
from being rebuilt during execution. Each run uses a private bus and isolated
configuration/data. This was not an old cached-library substitution.

The newly built desktop-feature application also passes its private
`--internal-check-copy`: Copy, Move, in-file cancellation with a completed prefix,
reopen and Undo. It was run from a frozen copy while the workspace build continued;
its SHA-256 is `e4b327b78bf8fd36ccdb16167cb45be31bb696d22519097b2d47efa64759691f`.
Receipts: `/tmp/canopa-copy-app-smoke.{log,json}`. This checks the linked new
application backend; it is not a new packaged-release or native GUI acceptance.

The full workspace suite remains red. Unselected later integration targets and
the downstream settings/configuration/vendor-library steps were not rerun after
the stop; prior wider Undo/Recovery limitations remain separately tracked. No
optimized desktop GUI build, cold-device/larger-than-memory/rotational benchmark,
resident startup/idle or full Files acceptance is claimed.

No wall-clock assertion is added to CI: the
repeatable engine workload and alternating frozen binaries provide the local
relative performance signal. The unchanged 500-item/20-second gate is retained.
Correctness tests guard every byte, both EOFs, cancellation before/during
verification, source/output edits after a chunk was already compared, mode 0000,
writer offset and the existing public mutation/crash behavior.

Storage pressure required reclaiming only completed test ELFs from the prior
migration's own runs, with Cargo/cache locks held. 34 Notify outputs (886,075,304
bytes) and 85 obsolete no-default core outputs (2,537,243,504 bytes, excluding
default-workspace/final-public outputs) were removed. Exact paths, sizes and
hashes are in `/tmp/canopa-copy-opt-{elf,core-elf}-reclamation.json`. No `.rlib`,
source, broad cache, failure fixture or private schema61 candidate was deleted.
Before the normal workspace build could replace the debug application, its
existing archive was checked to contain the identical baseline Files executable
(SHA-256 `26fc7825839f574146d2c22b819fe2883c7c35c658987771ff368b0c0688ce4a`);
see `/tmp/canopa-copy-protected-app.json`. Protected Niri outputs are untouched.

## Independent review

### Standards

No documented violation or compelling smell found. Verification stays in
fingerprint; preparation retains metadata, sync and lifecycle coordination. The
test hook has bounded thread-local lifetime and no production behavior.

### Spec

No verification weakening or unrequested scope expansion found. Full byte/EOF
equality with the same SHA-256 receipt, spanning snapshots, namespace checks,
metadata/sync and Undo/Recovery ownership remain. Review explicitly recognizes
unmeasured cold/rotational-storage performance as a limitation rather than a
verification issue. Public regression results are recorded separately above.

Review findings: Standards 0; Spec 0; no open issue within this scope. The full
Files goal remains open.
