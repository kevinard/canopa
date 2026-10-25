# Bound displaced Undo preparation and retention

Status: schema52 implementation verified on undo-displaced-inputs-04.json.
Continues ADR0142–0143. The complete Files goal remains active.

Two real public-worker crashes reproduced an unresolvable Undo Replace partial.
After a completed cross-filesystem Move, Undo accepted Replace for newer work at
the original location and died during its return copy. The edited partial remained
visible, but Keep was unavailable. The return root had original ownership proof;
the separate displaced-work folder had only legacy identity fields.

For a SourceRemoved restoration conflict, open the original-location entry with
O_PATH/NOFOLLOW before asking the question. Capture its version from that held
descriptor. After Replace, verify that selection again and prepare a private work
folder through actual opened parent/root handles. Its previous-entry generation
comes from the originally selected descriptor. Failed preparation uses verified
empty-only cleanup, including when unknown children arrive or a root is replaced.
It never recursively removes a pathname through TempDir drop.

Bind the actual preparation before copying or moving data. A private creator token
registers the exact initial Undo intent, original history-file generation, canonical
history step and displaced roots/selection in one checked transaction. Its source
descriptor remains absent until the actual return-copy reader extends that owner.
Only that initial transition may install the source proof; subsequent registration
must preserve it. Return and displaced roots must be distinct. Raw journals cannot
mint a pre-copy owner or upgrade old displaced records from current paths.
Schema52 prevents older executables from interpreting this preparation incorrectly.

Fresh Keep review checks the exact journal/binding/access bodies, return-root and
payload versions, displaced root generations and the selected previous entry. If
displacement has not occurred, it verifies the original still-unmoved selection.
Stale reviews, replacement roots and replacement previous entries refuse retention.
One checked transaction archives available leaves and both whole work folders,
including extra children, then retires only the captured journal and binding. It
moves or unlinks no files and leaves original Undo history available. A new Undo
request gets a fresh conflict answer; it cannot reuse the interrupted Replace.

Public acceptance kills the worker both after initial binding before copying and
after the newer occupant was actually moved, before return publication. It verifies
replacement-root refusal, safe empty preparation reconciliation, two restarts,
Keep/retry with both versions intact, whole-folder Recovery Copy with extra work,
and stale previous/root replacement refusal. The original moved destination stays
intact until the new Undo request runs. All 17 focused parents pass.

Final verification passes 536 backend tests across 20 targets (49 ignored helpers)
and 302 normal isolated desktop-feature library tests (29 ignored), with zero
failures or filters. Both required active/stopped batch restart gates, default
all-target Clippy with warnings denied, Files formatting and fixture schema/syntax
checks pass. All 889 captured build inputs stayed unchanged. Exact attempts are in
`.scratch/file-manager/undo-displaced-verification-01.json`; final source/evidence
is tracked by `undo-displaced-source-01-verification.json` beside it. Four failed
fixture roots retain 22 decoded and independently live-verified entries.

This completes a retention path for displaced Undo data, not automatic displaced
rollback or disposal. Those need complete durable one-shot handling. Cleanup after
a completed reversal and other generic Forward/Undo paths also remain. The broader
Undo/expiry, Trash/Restore, backend separation, product/media/device/accessibility/
physical, reliability/performance and native/workspace/release requirements stay
open. Unproved legacy displaced work remains copyable without cleanup authority.
