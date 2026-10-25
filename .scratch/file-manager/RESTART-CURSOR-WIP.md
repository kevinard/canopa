# Active transfer traversal persistence

The full five-area goal remains active. Previous source38 was verified unchanged
at this turn's start: `restart-owner-source-01.tar.gz`, SHA256
308c35520825de0576a48c906911cb93d51db1d41cd196da6ed0638d4323b966.

Schema39 adds one bounded normalized active-transfer cursor. Queue edits, newly
visited folders, failure/skipped/output rows, sparse saved receipts and small
progress counters persist incrementally. Row digests bind history, exact request
revision/body, operation, descriptor, role and key. Header counts detect missing
or extra tasks. History watermark and both journal/directory-access lanes must
match before converting interrupted live work to an independently sealed stop.
The existing original-object owner factory then reopens only proved originals.
A private engine token owns live cursor updates. A separate 64MiB cursor budget
keeps original per-request budgets intact; byte triggers avoid rescanning the
pending queue on each file. Immutable queued tails are protected by an actual
SQL refusal trigger in the public killed-worker fixture.

`restart-cursor-red-01.log` and `-red-02.log` repeat the real active-conflict kill
failure with an empty journal and a committed prefix. Their private retained
roots are listed in those logs and must remain. The new unignored active gate
passes Copy and actual cross-filesystem Move, both part orders, with edited
completed bytes/inode preserved and independent successor Undo (`core-03.log`).

`core-04.log` exposed a real Cut regression: an informational source remainder
prevented an end receipt, leaving the first part's cursor alive. `cut-01.log`
records its detailed isolated snapshot. Completed receipts now retain source
remainders. The final-file cancellation seam also lacked a receipt; real engine
regressions fail in `terminal-red-01.log` and `-02.log`. Terminal receipts preserve
cancelled/attention flags as well, with no work replay.

`core-06.log`: 16 transfer-continuation parents and 19 request-execution parents
pass (3+1 ignored helpers). Eight real killed-worker refusal states cover changed,
missing, borrowed and reordered rows, changed descriptors/watermarks, advanced
history, and failed stop-save rollback, each opened twice. The restored save case
also succeeds after the injected refusal is removed. `lib-01.log`:84 library
parents pass,7 helpers ignored, including the final-file cancellation regression.

The startup result-save retry owner seam fails twice in `save-red-01.log` and
`-02.log`: the stopped report saved but had no execution owners. Retry Save now
reopens proved originals after its result transaction and retains the activity
guard until explicit release. Its public regression passes.

## Final scoped verification

`restart-cursor-inputs-01.json` captures 861 Rust/Cargo/vendor/fixture paths.
`lib-02.log` and `core-10.log` through `core-18.log` pass 385 parents across 19
no-default-feature targets; 26 ignored entries are inventory helpers. All captured
inputs stayed unchanged in those batches.

Clippy requested one equivalent nested-if collapse in `transfer/restart.rs`.
The exact earlier file is retained as `restart-cursor-before-clippy-restart.rs`,
whose hash matches inputs01. Inputs02 captures the current source; its only
changed input is that style fix. `lib-03.log` and `core-19.log` recheck 120 parents
on inputs02, including both required unignored killed-worker gates and completion
cases. `restart-cursor-followup-01.json` records that relationship; the 385-test
source is not mislabeled as the later source.

Default all-target Clippy with warnings denied (`clippy-02.log`) and formatting
(`format-02.log`) pass on the current source. `desktop-lib-01.log` passes 165
parents,8 ignored, under private D-Bus/HOME/XDG directories; receipt
`continuation-core-1792639511-receipt.json` verifies all 861 inputs before/after.
Schema39 fixture syntax/hash is checked separately. `restart-cursor-verification-01.json`
records the exact profiles, summaries, source relationships, current gate binary
and unchanged protected app/Niri/earlier local-runtime gate.

The two original active-crash failure pairs remain intact and are decoded/verified
in `restart-cursor-red-fixtures-01.tar.gz` (30 entries), with their hash and paths
in its verification JSON. Final current source/proofs are preserved in
`restart-cursor-source-01.tar.gz`; inspect `restart-cursor-source-01-verification.json`
for its actual final hash and decoded member count. Existing checkpoints remain.

No native, full workspace/release, Shell/Notify or physical-device acceptance is
claimed. The earlier intermittent paused-video/seek timeout remains undiagnosed;
a passing isolated library run does not resolve it.

Remaining: interrupted filesystem steps whose history advanced beyond the cursor,
copy-publication/source-removal origin facts, directory publication/cleanup and
saved-container phase advancement, queued lineage/admission, legacy handling,
large restored reports/paged execution owners, sanctioned Undo/relocation/expiry,
and all other restart families and goal areas. The existing watermark refusal is
a safety boundary, not completion of filesystem-step continuation. Retain the
source27 application, compatible Niri and prior required gate; do not overwrite
those binaries or erase any failure datasets/proofs. No commits.
