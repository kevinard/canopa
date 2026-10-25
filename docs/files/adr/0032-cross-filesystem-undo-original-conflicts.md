# 0032: Conflicts at a cross-filesystem move's original location

Status: accepted under delegated implementation authority; verified for the scope below.

Returning a cross-filesystem move recreates its source from the verified destination
copy. If newer work occupies the original name, interactive Undo now offers the
same Replace, Keep both, Skip and Cancel choices as other restorations, showing
both real files' metadata. Keep both remains the Return-key default. Replace is
available for files and links and preserves the displaced newer work in Recovery.
Returning links preserves their raw target bytes without following their targets.

The return is fully prepared on the original filesystem before displacing the
newer occupant. Preparation retains the Undo step, selected relocation and
protected displacement record in its durable intent, including while the copy is
incomplete. The prepared source's physical identity and fingerprint must still
match the recorded destination version; its original parent is checked again
before installation. This does not replace the remaining directory-descriptor
and original-media authority work.

Restart recognizes the prepared returned identity at the actual selected path.
It applies source ownership and Keep both relocation to that path, archives
any displaced newer work, consumes the completed source-return step and clears
its intent atomically. An incomplete return rolls back any displacement before
clearing its intent and cleaning its own preparation. No wire/schema change is
needed; the existing Undo journal carries these fields.

A cross-filesystem move also has an earlier destination-copy step. Skipping its
return must keep that copy intact. Within the batch, an unresolved current path
blocks earlier frames that act on that path or move an ancestor away, including
a same-filesystem rename; other paths continue normally.
Created destination containers also remain when a skipped child still needs them.
Skip and Cancel therefore leave the move retryable with its data and saved prior
destination intact, including after reopening history. This also preserves frame
ordering when a batch contains repeated destination names.

Public tests use two actual filesystems and the agreed transfer/Undo/Recovery
interfaces. Coverage includes older Copy Undo after Keep both, protected Replace,
raw link targets, batch Skip, stale occupants/parents and process kill/restart in
100-file Keep both and Replace batches. Native acceptance also requires different
filesystem device IDs and isolated temporary directories; it uses normal cut,
paste, Undo and Recovery UI. Exact verification is recorded in the implementation
ledger as it completes.

This decision covers conflicts at the original return location. Changed or
unavailable cross-filesystem destination copies still require their own review
and continuation work. Directory conflicts/Merge, broader dependency decisions,
phase-specific interrupted recovery, identity bounds and device authority remain
part of the full product goal.

Final verification passes 166 core tests, with thirteen child helpers exercised
by their parents. Mixed same-/cross-filesystem batches verify that Skip blocks
older file and ancestor-folder returns while preserving other paths. Build and
all-target Clippy with warnings denied pass. Native 48 dark and 49b light at
1.25 scale each pass twelve checks on different actual device IDs and confirm
secondary-directory cleanup. Existing Undo regression 50 passes 31 checks and
moved-item regression 51 passes 24, totaling 79 with clean GTK logs. Rendered
inspection confirms readable two-version conflict metadata in both themes.

The first native attempt typed Space inside a filename; its selection input was
corrected to Home. A light attempt exposed the fixture racing Recovery copy's
Operations focus handoff; it now waits for that foreground transition. These
needed no application UI changes. Final light 49 received external SIGTERM and
cleaned up; 49b is the successful rerun on the same binary. First-iteration proof
predates the final mixed-batch guard and is recorded separately. Exact hashes,
logs, interrupted attempts and remaining scope are in `cross-undo-verification.json`
under the file-manager scratch directory.
