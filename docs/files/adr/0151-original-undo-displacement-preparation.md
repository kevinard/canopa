# Original Undo displacement preparation

Status: schema59 implementation and diagnosis in progress. On
`move-return-inputs-06.json` (903 captured inputs), the complete Undo crash group
passes76 parents with seven ignored workers; its six new direct displacement
parents pass. Eight unfiltered integration targets pass82 parents and fail two,
with seven ignored workers. The existing500-item Undo gate now takes29.84 seconds
against its20-second limit, and the deliberate legacy fixture with removed
history bindings refuses automatic saved restoration. Modern control passes.
These remain open; there is no current full backend, static, native or release
acceptance. The complete Files goal remains active.


Two public additional-pass tests exposed Undo Replace creating a saved folder
without its original generation. Later directory Merge correctly refused to adopt
that folder. The fix starts with the actual conflict observation, rather than
weakening Merge's admission checks.

All supported Undo Replace conflicts now open the selected entry and its actual
parent before the question. Preparation checks that observation, creates the work
folder through the held parent descriptor, records the original selected entry,
and registers the exact Undo task and independent original-history binding before
moving the selected conflict. The live displacement uses the verified parent and
created-folder descriptors, followed by syncing both directories. Registration
covers direct Rename, Trash and saved-only return shapes as well as existing
cross-filesystem return and changed-copy preparation.

An unused or displaced preparation enters the existing consumed compensation
protocol. A raw rollback rename cannot grant a reused previous name another
return. Original-root checks and fresh Recovery Keep apply to direct preparations;
legacy journals without independent proof remain protected. No copied-source
reproduction rights are granted to direct work.

A saved-return task atomically supersedes its exact original preparation binding.
Completion uses the recorded incoming identity when it updates older source-owner
contexts; a later source-name occupant cannot become the older Copy's property.
Ordinary Copy completion stays off the additional journal-decode path. Binding
retirement verifies absence, so an ignored DELETE cannot silently lose an owner.

The reviewed saved-return permission case now returns its originating live
filesystem error while retaining uncertain consumed work for Recovery. Read-only
whole-folder export remains available after the saved name is consumed. Its
public regression passes Copy, fresh Keep, restart, history expiry and a second
whole-folder export while preserving the returned incoming file and saved edits.

This does not finish incoming Move name consumption, unproved legacy work,
Trash/Restore restart or all interrupted cleanup. Backend extraction, product and
device/accessibility acceptance, startup/focus/performance, and complete workspace
and release integration remain open.

Diagnosis preserves source06 in a903-member decoded/live-verified archive. The
failed timing fixture retains all500 newer destination versions; its1507 entries
are archived and independently live verified. A private repeat captures both
modern and legacy deferred-return roots (26 entries), also archived/live verified.
A private timed build reproduces29.50 seconds; repeated serialization accounts
for7.8 seconds of its first pass. Reusing registered canonical bytes with the
existing normalized fallback reduces the measured total to26.89 seconds, still
above the unchanged gate. No private profiling/encoding experiment is installed
as production code. See the current handoff and `.scratch/file-manager` receipts.
