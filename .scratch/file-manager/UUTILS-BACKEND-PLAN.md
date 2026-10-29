# Implement a uutils-backed copy backend for Canopa Files

Status: scoped Copy implementation under ADR0153; see UUTILS-IMPLEMENTATION.md for evidence and remaining limits.
Prepared: 2026-10-25. Workspace: `/home/kevinard/projects/canopa`.

Subsequent user decision: [ADR0155](../../docs/files/adr/0155-trust-upstream-copy.md)
trusts successful upstream Copy and hashes its output once for Undo. It supersedes
the independent source/output content-comparison requirement below; Recovery and
destructive-action safeguards remain. See [current evidence](COPY-TRUST-PERFORMANCE.md).
The later [ADR0156](../../docs/files/adr/0156-fast-content-fingerprints.md) also
removes development-history compatibility and uses XXH3-128 content fingerprints.

## Request and intended outcome

Replace the custom content-copy implementation with maintained uutils code, while
preserving Files' user behavior and its guarantees around completed work, Undo,
replacement data, and original files/devices. Reduce duplicated filesystem logic
and recovery coordination as paths migrate. Do not put a new backend underneath
every old special case indefinitely.

The user requested this plan for another agent after reviewing the existing
architecture. They support investigating both Rust library calls and subprocesses.
The recommendation is a small Files-owned worker that links selected uutils code;
the exact interface must pass the feasibility gate below. This is a proposed
implementation direction, not a claim that upstream already provides the needed
file-manager interface.

Start implementation with the bounded proof in Phase 1. Do not resume adding the
private, operation-specific Move recovery protocol as the default next task.

## Read first and preserve the working baseline

- [Root instructions](/home/kevinard/projects/canopa/AGENTS.md),
  [context map](/home/kevinard/projects/canopa/CONTEXT-MAP.md),
  [Files instructions](/home/kevinard/projects/canopa/crates/canopa-files/AGENTS.md),
  [Files domain](/home/kevinard/projects/canopa/docs/files/CONTEXT.md).
- [Product brief](/home/kevinard/projects/canopa/.scratch/file-manager/PRD.md),
  especially file operations, conflicts, cancellation, and background lifetime.
- [uutils research](/home/kevinard/projects/canopa/.scratch/file-manager/UUTILS-LIBRARY-RESEARCH.md)
  distinguishes verified release interfaces from `main`-only observations.
- [Implementation ledger](/home/kevinard/projects/canopa/.scratch/file-manager/IMPLEMENTATION.md)
  and [current recovery handoff](/home/kevinard/projects/canopa/.scratch/file-manager/MOVE-RETURN-NEXT.md)
  hold the historical source manifests, test receipts, protected artifacts, and
  known failures. Reference them; do not recreate their bookkeeping.

At preparation, the installed source uses schema60. The schema61 incoming-Move
experiment is separate, under the path recorded in
[incoming-return-candidate-location.json](/home/kevinard/projects/canopa/.scratch/file-manager/incoming-return-candidate-location.json).
It has only focused private verification and known design gaps. Do not copy it
into production. Its crash reproductions can supply useful regression cases.

At plan preparation, the workspace contained unrelated dirty Notify, integration,
packaging, and vendor work, and the Files crate was untracked. The user then
requested an implementation checkpoint and test-directory cleanup. Consult
`git status`, `git log`, and
[the local-artifact note](/home/kevinard/projects/canopa/.scratch/file-manager/LOCAL-ARTIFACTS.md)
for the resulting baseline. Inspect actual working files before editing; never
clean or reset unrelated work. Preserve retained failure fixtures and the
protected Files/Niri executables listed in the current handoff. Generated local
evidence is deliberately excluded from Git.

## Architecture and scope

```text
Iced / other clients
        |
OperationJobs: request lifetime, questions, progress, cancellation
        |
OperationEngine: traversal, policy, journal, publication, history/Undo
        |
CopyBackend: prepared source + owned temporary destination -> copy outcome
        |
headless worker using pinned uutils library code
```

Files continues to own selection identity, filesystem/device checks, conflict
decisions, staging ownership, durable completion, atomic publication, source
removal, retained replacements, Undo, and Recovery. The CopyBackend owns content
transfer and the chosen metadata policy. No renderer types cross its interface.

Use uutils where it removes maintained behavior, particularly optimized copying,
sparse files, reflinks, and metadata helpers. Keep direct OS rename/unlink
primitives for the identified entries Files controls. There is no benefit in
spawning `mv` for each atomic rename.

**Do not delegate a complete cross-filesystem Move to `uu_mv::mv` or `mv`.** Its
copy/delete fallback offers no host commit hook between destination completion and
source deletion. Compose Move from the shared copy path, publication, and verified
source removal instead. Trash metadata and Undo are also Files responsibilities.

Navigation/search/preview extraction, unrelated focus fixes, GTK retirement, and
the rest of the five-area goal are outside this migration. Package integration
and regressions caused by the new backend are included. Migration completion does
not mean the whole restart-recovery goal is complete.

## Non-negotiable behavior

1. Keep completed entries and their Undo history after cancellation or restart.
   Retry unfinished files from byte zero if needed; byte-offset resume is not a
   requirement. Preserve useful partial data if the original source is no longer
   available or its ownership cannot be safely reconciled.
2. A cross-filesystem Move retains its selected source until the destination is
   published, synced, and durably recorded. Cancellation before that point leaves
   the source intact.
3. A pathname is descriptive, not evidence of original-file or device identity.
   Preserve the existing descriptor/original-generation checks. A fresh lookup
   after a conflict question or restart must not silently authorize a replacement.
4. Copy into private, owned storage. Retain overwritten destination versions for
   Undo. Publication refuses an unexpected occupant; user approval to Replace is
   tied to the reviewed version, not to whoever later occupies that name.
5. Unknown native-call outcomes are reconciled from evidence or exposed through
   Recovery. Never repeat an uncertain destructive step merely because its old
   pathname exists again.
6. Preserve raw Unix filename bytes, symlink semantics, directory merge behavior,
   retained-source leases, live Retry/Continue/Abandon, and operation lifetime
   after all Browser Windows close.
7. An unacknowledged or still-running worker cannot race restart reconciliation or
   cleanup. Establish that the writer is gone, or retain execution ownership and
   wait. Files must never clean a stage while another process can still write it.

## Phase 1 — prove and select the reuse interface

Deliver one runnable, private-file prototype and a short decision. Reuse the
research; investigate only the selected-version gaps rather than starting another
broad library survey.

1. Verify the actual published crate/release, toolchain compatibility, required
   features, license notices, and dependency footprint. Research inspected tag
   0.12.0, but that is not proof that every cited `main` helper is available in the
   corresponding published package. Pin the selected source/version in the root
   dependency declarations and lockfile when integrating.
2. First evaluate `uu_cp` behind the worker. Its public `copy` interface takes
   paths; its options expose terminal progress, not supplied progress/cancellation
   callbacks. Prove how it will operate on the selected source and owned output
   without reopening unrelated user pathnames. Do not assume `/proc/self/fd`
   spelling automatically solves canonicalization, symlinks, or metadata copying.
3. If that interface cannot preserve the contract, evaluate the exported
   open-file copying/metadata helpers or the smallest upstream extension needed
   to expose them. Verify their real contracts, including sparse/reflink support.
   Do not claim all of `cp`'s functionality from merely calling `copy_fast`.
4. Exercise a regular file, symlink, non-UTF-8 filename, restricted parent, replaced
   source/parent, and a killed worker. Demonstrate useful byte progress during one
   large file and cancellation of that file. Terminal animation parsing and an
   uncancellable worker thread do not satisfy this gate.
5. Select one production implementation. Prefer a minimal headless worker linked
   to the selected library. Direct linkage is acceptable if cooperative
   cancellation, reporting, and process-global state are actually isolated.
   A pinned command subprocess is acceptable only if it meets the same identity
   and reporting contract. Do not ship three interchangeable backends.

If a small upstream patch is required, follow
[vendor maintenance](/home/kevinard/projects/canopa/docs/vendor-maintenance.md):
one root `vendor/` copy/overlay, attribution, provenance, and a reviewable patch.
Do not transplant uutils source into the application crate. Keep any patch narrow
and suitable for upstream submission. If none of the evaluated interfaces can
meet the contract without recreating a transfer engine, document the concrete
failure and the smallest missing upstream interface; do not silently weaken the
requirements or continue an unbounded fallback design.

Exit: a selected, demonstrated interface, dependency plan, process placement, and
an ADR recording the choice and the old decisions it supersedes. Do not defer all
hard interface questions until after a production migration.

## Phase 2 — add the CopyBackend and worker lifetime

Introduce a small private module at the prepared-copy seam. Its inputs represent
the already-selected source, already-created owned output, metadata policy, and
job cancellation/progress channel. Its result distinguishes completed staging,
cancelled work, a source that changed, and an I/O failure. It cannot publish a
destination, delete a source, edit history, or answer conflict questions.

For the preferred worker design:

- Use a headless executable/entry point with no Iced, desktop activation, media,
  or user-bus initialization. Choose the smallest workspace arrangement that
  achieves this; do not overload `canopa-file`, whose current purpose is bounded
  reads for shared assets/configuration, with Files-specific scheduling.
- Use a bounded, versioned local protocol with a job identifier, progress,
  completion/failure, and an unambiguous terminal outcome. Pass necessary open
  descriptors explicitly; raw path bytes must survive any remaining path fields.
  Descriptive wire records never grant durable mutation rights by themselves.
- Keep diagnostics separate from structured results. Check the actual per-job
  result and process exit. uutils aggregate return values/global exit status are
  not per-item receipts. Configure noninteractive behavior explicitly.
- Define cancellation, owner death, worker death, and acknowledgement-loss
  behavior. Prevent orphan writers. Reap children and drain bounded diagnostics;
  closing a Browser Window must not kill an accepted Transfer.
- Avoid spawning a full GUI executable per file. Measure process startup and
  state reset before deciding whether a small reusable worker is appropriate.
  Prove that one job's global status/options cannot contaminate the next.
- Locate the packaged worker deterministically. Never execute an arbitrary
  `cp`/`mv` from PATH or construct a shell command from filenames. Missing or
  incompatible workers fail before user data is changed.

Specify the supported metadata policy: ordinary mode and timestamps, ownership
where appropriate, ACL/xattr behavior, symlinks, sparse allocation, and hard-link
groups. Preserve existing behavior and add deliberate improvements with tests.
Do not adopt `cp -a` defaults blindly or report successful preservation when a
required attribute failed. Define understandable behavior for filesystems lacking
optional metadata features and for unsupported special file types.

## Phase 3 — migrate one complete Copy path

Start at `fingerprint::prepare_copy` / `prepare_pinned`, retaining stage
registration, original source/output identities, cancellation, sync, and the
existing publication/history contract. Keep source validation and hashing logic
separate from the data mover; the present module mixes those responsibilities.

Replace the handwritten content-copy loop with the selected backend. Preserve
the meaning of existing fingerprints for Undo and cross-filesystem Move
verification. uutils does not automatically produce our SHA-256 receipt. Account
for any extra hashing pass in measurements; do not remove content checks to make
the performance figures look better. A later change to fingerprint policy needs
its own justified design and acceptance.

Prove ordinary Copy through OperationJobs, conflict Replace/Keep both/Skip,
cancellation with completed entries retained, and Undo. Include a crash after
worker completion but before the host records completion: a success message alone
does not make publication durable. Only a verified complete stage may publish.

Keep the existing journal representation where its meaning is unchanged. If a
new durable state is required, state the invariant first and make one explicit
compatible migration. Do not add a parallel family of uutils-specific recovery
tables and pending variants for each operation.

## Phase 4 — converge consumers and recovery

Route all actual copying through the same prepared-copy implementation:

- Ordinary and recursive Copy, directory merges, batch retry and continuation.
- Cross-filesystem Move, retaining verified post-publication source removal.
- Recovery Copy/export and retained saved-container copying.
- Cross-filesystem Undo/return paths that genuinely need a copy.

Audit call sites rather than assuming every path already uses `prepare_copy`.
Preserve hard-link-group semantics across per-entry jobs where supported; resetting
uutils state on every file must not silently destroy that behavior.

Use one copy lifecycle across these consumers: owned preparation, content/metadata
completion, host durability/publication, logical history completion, and owned
cleanup. A completed file stays completed; only unfinished entries are retried.
Separate logical Undo records and Recovery catalog entries from temporary worker
execution state. Preserve their existing ownership and lease relationships.

Recover old schema60 operations according to the evidence they already contain.
Do not reinterpret missing legacy proof as permission to run a new worker against
current paths. If a database upgrade is needed, older executables must refuse
unsupported work safely. Test reopen/upgrade with actual interrupted fixtures.

Delete superseded data-copy code and duplicate coordination when each consumer
migrates. Keep legacy decoding/reconciliation only where existing persisted work
requires it. A temporary rollout switch must have a removal step; shipping the
new backend with silent fallback to the custom copy engine is not completion.

This phase should consolidate the shared copy lifecycle, not rewrite every Undo
and Trash protocol at once. Carry known generic incoming-Move/Undo, legacy, and
cleanup gaps forward explicitly. The private incoming-return reproduction is a
regression reference, not evidence that this migration solves those gaps.

## Phase 5 — verification and package integration

Use temporary files and isolated runtimes only. Do not mutate real user folders,
host mounts, or block devices. Test through the public operation interfaces, with
worker fault injection for the native/recording gaps; avoid tests that only
mirror protocol serialization.

| Area | Required evidence |
| --- | --- |
| Content and metadata | Empty/large files, raw-byte names, symlinks including dangling links, mode/time policy, ACL/xattrs where supported, sparse files, hard links, restricted directories, unsupported types. |
| Conflicts and Undo | Replace retains the old destination; Keep both and Skip; folder Merge; later edits; shared Undo history; retained-source leases and Recovery export. |
| Cancellation and errors | Cancel during one large file and between files; source read/output write errors, full destination, unavailable device, missing worker; completed work survives and unfinished Move sources remain. |
| Crash/restart | Kill worker and Files during copy, after copy before acknowledgement, before/after publication, before history completion, before source removal, and during cleanup. Recover without duplicate publication or destructive replay. |
| Replacement refusal | Replace source, source parent, destination parent, staging root/payload, or mounted device at relevant cuts. Reintroduce old names/hard-link aliases after a completed rename. Preserve all unrelated data. |
| History compatibility | Reopen current histories and representative schema60 interrupted work; preserve saved versions, exact task/history bindings, and user-visible Recovery choices. |
| Lifetime and packaging | Operations survive closing all windows; no orphan writer after owner death; packaged worker found without PATH dependence; missing/version-mismatched worker fails safely. |

Relevant existing targets include `operations`, `transfers`, `operation_transfers`,
`operation_jobs`, `operation_transfer_resume`, `transfer_stage_recovery`,
`transfer_source_remainders`, `directory_permissions`, `request_source_retention`,
`recovery_ownership`, `history_upgrade`, `identity_context`, and the changed-copy,
saved-version, replacement, directory, and cross-filesystem Undo targets under
[Files tests](/home/kevinard/projects/canopa/crates/canopa-files/tests).
Retain useful existing subprocess crash tests. Add fault cuts around the worker
handoff and host commit, including completion-message loss and ignored/refused
journal writes. Worker failures must not clear the old durable owner.

Run focused checks as each seam changes, then the affected complete targets and
Files library/default-feature checks in the isolated environment documented by
the existing handoff. At final integration run:

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
bash scripts/check.sh --core
bash scripts/check.sh
```

After dependencies have been fetched/locked, local Cargo checks can use
`CARGO_INCREMENTAL=0 TMPDIR=/tmp`, `-j1`, and `--offline` as in the existing
verification workflow. Do not run concurrent Cargo mutations or rebuild a live
test-helper executable. Preserve protected binaries before build outputs replace
them; do not clear broad caches or failure fixtures to reclaim storage.

Record a small before/after performance set: large-file throughput, many small
files, sparse/reflink cases, cancellation latency, worker overhead, and startup/
idle impact. Separate copying from hashing and fsync costs. Keep the existing
500-item/20-second Undo gate unchanged; it is already known to fail on some
baseline measurements. Report inherited versus introduced failures accurately and
do not claim full acceptance while that gate remains open.

Include the worker in
[release packaging](/home/kevinard/projects/canopa/scripts/package-release.sh),
[release smoke checks](/home/kevinard/projects/canopa/scripts/smoke-release.sh),
CI/build target selection, and dependency attribution/license notices. Smoke-test
an actual private Copy/Move/cancel/restart using the packaged backend; `--help`
alone is insufficient. Run applicable native Files checks and Shell/Notify
regressions for shared packaging/dependency changes. Existing native acceptance
and full release work from the wider goal remain separately tracked.

## Source map

| Location under `crates/canopa-files/src/` | Responsibility |
| --- | --- |
| `operation_jobs.rs` | Accepted job lifetime, progress, questions, cancellation. |
| `operations.rs` | Engine interface, logical Change records, pending states. |
| `operations/transfer.rs` | Traversal, conflict policy, Copy/Move orchestration. |
| `operations/fingerprint.rs` and `fingerprint/source.rs` | Current copy loop, held source selection, versions, hashes, and metadata. |
| `operations/staging.rs`, `replacement.rs` | Owned output preparation, publication, saved destination. |
| `operations/entry_move.rs`, `source_removal.rs` | Same-filesystem claims and post-copy source removal. |
| `operations/store/`, `undo/`, `recovery/` | Durable execution/history, reversal, retained data and leases. |
| `operations/trash.rs` | Trash payload and `.trashinfo` coordination. |

## Completion and next-agent instructions

The migration is implemented when production copies use the selected pinned
uutils implementation, the custom data-copy loop and temporary fallback are
removed, the shared copy lifecycle serves the listed consumers, worker lifetime
and package integration are verified, and the changed behavior passes its
acceptance cases. State any inherited failures and unrun checks precisely.

Deliver the code, focused ADR, relevant tests, concise before/after measurements,
and updated ledger/context/package documentation. List deleted code and remaining
custom responsibilities so the user can judge whether the migration actually
simplified maintenance. Do not create a new archive or ADR for every small fix.

The wider five-area goal remains active. In particular, successful uutils adoption
does not by itself finish generic incoming-Move Undo, Trash/Restore restart,
legacy cleanup, backend separation, native acceptance, or release integration.

Suggested skills: `codebase-design` for the prepared-copy module and its small
interface; `implement` for executing the plan; `diagnose` if a reproduced failure
needs investigation. Use the existing research note first. Preserve user
authorization and repository instructions when applying any skill.

Suggested prompt to the implementing agent:

> Implement the uutils migration described in
> `/home/kevinard/projects/canopa/.scratch/file-manager/UUTILS-BACKEND-PLAN.md`.
> Start with its bounded feasibility proof, then migrate one complete Copy path
> and its consumers. Preserve the working-tree Files source, unrelated edits,
> original-file/device checks, completed work, and Undo/Recovery data. Reuse
> upstream copying while consolidating the shared copy lifecycle. Report evidence
> and remaining limitations; do not count this migration as completing the full
> Files goal.
