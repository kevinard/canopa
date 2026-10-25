# 0119: Retain the reviewed replacement entry and journal its return

Status: scoped verification; restart execution ownership remains unfinished, 2026-10-22.
Continues ADR0118. The complete Files goal stays active.

The old installer checked destination metadata, then renamed it by pathname.
Real backup-boundary tests replace the destination parent and recreate the work
name there. A hard link to the reviewed old file passes its moved-version check.
Copy mutates that foreign folder before rollback; Move installs the source there
and reports completion. The two retained red fixtures demonstrate both failures.

Keep the final destination entry and followed parent open before conflict review.
Use their descriptor-derived version and available parent/object generation facts.
Claim the old entry into `previous` through the actual parent and private work
descriptors with NOREPLACE. Unclaimed means no backup rename was attempted; it
permits cleanup of proved generated work without inspecting or returning a
changed destination. Persist Ready only after the final preflight and before the
backup syscall. Saved and Returning phases distinguish a verified old
version from an actual return subject. Save return direction before moving it back;
an occupied name or uncertain original keeps both data and journal intact.

Copy replacement owns its publication phase and actual generated-output facts.
Same-filesystem Move composes the replacement with its existing entry claim rather
than turning a Move into a copy. A refused source return also returns the saved
destination through its original parent. Completion keeps replacement facts in
Retained and actual output facts in Copied, while cleanup receives empty-work
rights. Returned or edited staging cannot become fresh publication authority.
Recorded publication restores metadata while preserving newer output bytes.

Schema34 gates these roles. Recovery must match the replacement facts, normalized
saved-work record and the original parent/work/object generations before mutation.
Unsupported original facts permit no reconstructed restart authority. Derived
Undo and legacy saved versions do not invent new replacement facts.

The scoped replacement tests exercise66 actual SIGKILL cases (claim, publication,
return, changed output/backup, original-object substitution and unclaimed target
replacement) with two opens each, plus the original live parent races. Selected
no-default-feature backend checks pass360 parents across19 targets; the required
restarted Retry/Continue test remains the sole failure. Default desktop-library
rerun passes147,6 ignored, and all-target Clippy/format pass. The first desktop
run had an undiagnosed video timeout; its isolated rerun and full rerun pass on
unchanged captured sources. This does not establish a media reliability fix.

Broader crash verification, original-fact consumption by general Undo/expiry,
saved-container movement, decomposed empty-folder removal, complete operation/step
binding and actual stopped execution ownership remain open. Keep the required
unignored Copy/real cross-filesystem Move Retry/Continue gate and the full later
backend, product/device, reliability/performance and release work intact.
