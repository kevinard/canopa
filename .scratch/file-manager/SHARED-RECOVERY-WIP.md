# Shared Recovery ownership checkpoint

Whole file-manager goal active; classification progress. No agents/blocker.
Baseline main65e7cc245dc52607196bae6330f4a92da49918e2; .git read-only.

ADR0048/schema16 context work is verified at its saved snapshot: core282/19
helpers, Clippy/build/format, static checks; final native99 light1.25/78852 and100
dark/25771 each13 checks,16 captures inspected, private destinations removed.
Desktop2ae680c7f0229f85f0d051bcd685b27c067d2598d10cb09528908c4af4659c59.
Proof: context-epoch-verification.json, context-epoch-source.json150 paths and
context-epoch-compiled-source.json98 paths. Native98's remote-focus assumption
was replaced with explicit user tile selection; no app change for that timeout.
CONTEXT-EPOCH-WIP.md has detailed evidence and all prior source decisions.

## Current tracer

New public tests: crates/canopa-files/tests/recovery_ownership.rs. Real private
Copy-replace prepares an actual active backup. Closed-engine SQL prepares v15
shared-owner input: an archived record of that same saved container, optionally
protected, with appropriate old derived namespace/stage indexes. New16 derived
structures are removed and version set15; optional removal of the old operation
leaves archived-only owners. No private DB output assertions: observe public
Recovery/Undo/copy-out and real filesystem bytes.

Three cases: archived unprotected + active; archived unprotected + protected;
two archived unprotected with no active. Startup/trim must preserve needed data
and dispose unchanged unprotected ownership once without Unavailable remnants.

54074 terminal101: all3 initial parents fail in0.14s (shared-recovery-red.log).
Active backup and protected shared archive become Unavailable; two unprotected
archived owners leave a dangling Unavailable record after one deletes the data.
56208 terminal101 (shared-recovery-current-red.log): current16 input also loses
the actual saved file (ENOENT) during startup, disproving migration-only and
catalog-only causes. The first parent now tests both current16 and prior15.
32013 terminal101 (shared-recovery-additions-red.log): a saved folder with extra
user work loses its ordinary previous file before rmdir fails, leaving the
archive Unavailable. An added changed-shared-version parent guards against
silently discarding protected metadata. Five parents now exist.

User-visible ranked hypotheses were shown before the current-input probe:
1 per-record cleanup ignores shared owners;2 upgrade loses bindings;3 stale
snapshot without actual deletion. Current16 ENOENT confirms1.

First fix: factor read-only require_retained_expiry from unlink. Keep flags,
versions, identified container/payload and known-contents checks; reject special/
directory payloads without unlink. Reuse saved_stage::require_only so additional
work prevents both payload expiry and metadata retirement. Store's stage owner
lookup resolves other active/archive records to the same identified stage path.
For eligible unchanged data with another owner, discard only this redundant
archive metadata; last owner expires actual data once. Protected/edited/
unavailable/extra-content records remain. Shared absent payloads keep metadata.
Schema remains16. No journal/disposal/FD-authority completion claim.

Formatter29373 terminal0. First fix25406 terminal0: five parents pass in0.31s.
Two alias probes2841 terminal101 (shared-recovery-alias-red.log): actual public
Copy-replace through a parent alias creates an active owner whose lexical stage
path differs from the archive's real path. Both live and temporarily absent alias
lose saved bytes under the first strict-path guard. The fixture obtains its
canonical saved path from public Recovery; only the archived input uses that
real path, while active history comes entirely from the real alias operation.

Second fix streams indexed owners and stops at first match, instead of eagerly
allocating every owner for each GC row. A matching stage identity can protect
bytes even through a missing owner alias. Available matching physical containers
through either path allow redundant metadata retirement; unavailable owner paths
preserve this available record too. No global inode adoption/deletion authority.
Seven parents now cover live/missing alias, current/prior input, active/protected/
changed/additional contents, last-owner expiry and copy/Undo/reappearance.

Formatter8395 terminal0. Focused80603 terminal0:41 parents pass (7 ownership,
6 Recovery,5 upgrades,23 saved-stage), one parent-invoked helper definition with
observed boundaries. Aliases green alongside all first-fix cases. No temporary
production instrumentation was added for this phase.

Full core88150 terminal0:289 parents pass,19 parent-invoked helper definitions;
shared-recovery-final-core.log. All seven ownership regressions pass alongside
the original context, upgrade, transfer and observed interruption suites.
11298 terminal0: all-target Clippy45.45s/default build37.36s/format5.82s pass.
New desktop SHA
3bdcd3ce11a4611f577df16889160091df6e5140d36a6d06f60ebe3b934f03e4.
All99 compiled source paths unchanged. Native101 dark/83970 terminal0 in
native-shared-recovery-101-dark:14 checks, actual35/2050 devices, both GTK logs
clean, private secondary removed. All8 captures inspected via contact sheet plus
full-size04/06/08. One protected row remains after injected unprotected shared
ownership expires; saved contents/filename preview and copy stay intact through
KeepBoth. Native102 light1.25/20828 terminal0:14 checks, devices35/2050, both GTK
logs clean, private secondary removed. All8 captures inspected via contact sheet
and full-size04/06/08; one protected row remains after shared expiry and return,
all three previews show protected text/original filename, chooser remains clear.
Both runs total28 checks/16 captures.153 tracked/untracked source paths whitespace
clean. No Rust edits after snapshot.
99 build/Rust source paths snapshotted in shared-recovery-compiled-source.json;
Python syntax, maintained matrix and tracked whitespace pass. No Rust edits
after the snapshot. No current native completion claim. Prior2ae680... belongs
to ADR0048 proof only. Native context fixture now
injects an unprotected duplicate of its
protected archive in stopped-app v15 input, copies the proper derived old indexes
and tests that startup expiry leaves one protected, previewable/copyable version.
Expected maintained native checks now14; no new native/build claim yet.

## Ownership implementation and remaining work

The reproduced Store::collect_garbage decoded each archive and called
expire_retained without consulting store/stages.rs owners. It now calls the
ownership guard before retiring metadata or unlinking. Eligibility retains the
recovery flag, stored version, identified stage and payload checks, and verifies
known contents first. Stages::owners indexes active
and archived keys by physical saved-container identity; matching actual stage
path/identity remains necessary when resolving owners.

Diagnosis confirmed per-record unlink ignores the other owner. Read-only
eligibility and actual unlink are now separate; other identified reachable owners
allow redundant unprotected metadata retirement without physical deletion.
Protected/changed/unavailable/extra-content records remain. Missing aliases keep
the available record too; reappearance restores normal cleanup/older Undo. The
owner check streams the index and stops early. Do not infer a disposal policy
from matching names or an inode elsewhere. No temporary instrumentation remains.

ADR0049 and shared-recovery-verification/source manifests record current scoped
proof. All sessions terminal;99 compiled source paths unchanged. Continue
unusual/directory/extra saved
payloads, explicit private selection, legacy decomposition, durable jobs,
authority, desktop/focus and release/workspace/review/commit work in the entire
ledger. Full goal remains active; do not claim full Copy/Move/Undo or product done.

## Next directory-Recovery tracer (not yet run)

Use a real public Copy-replace to produce an active saved file. Obtain its actual
saved path through public Recovery. Move that private test backup aside and
create a directory at the same saved path with nested user files and a raw link.
This is unexpected newer saved work, not permission to delete it. Reopen/expire
the original operation through ordinary public history and inspect Recovery:
the directory must remain available and Recover a copy should create the chosen
new folder with its contents, without consuming the retained version or changing
the current destination/held original file. This needs no private DB outputs.
Current recover_copy uses fingerprint::prepare_copy, which supports files/links
and may reject the directory; run the actual public tracer before designing a fix.
Preserve bytes/names, interruption journals, Undo of copied output and safe
handling of new output work. Do not claim directory Recovery from preservation
alone. Explicit protected disposal, extra-content recovery, legacy decomposition,
durable jobs, authority, focus/integration and the full ledger remain required.
