# 0117: Claim cross-filesystem Move sources through their opened parents

Status: implemented for ordinary cross-filesystem leaf source removal; broader
restart execution and acceptance remain open, 2026-10-22.
Continues ADR0116. The full restart and product/release goal remains active.

Two deterministic tests at the real mutation boundary expose unsafe source
cleanup. Replacing the source parent and recreating the stage name moves the
replacement parent's file into that foreign stage. Replacing the quarantined
payload after fingerprint verification makes unlink delete the replacement.
The original copied source survives elsewhere in the private fixture, but the
unselected entry has been changed or deleted. Both red fixtures are retained.

Keep the opened source leaf and parent from copy preparation until removal.
Create private work through that actual parent, journal a Ready source claim,
then claim the name with descriptor-relative NOREPLACE. Verify the actual leaf,
source bytes, source namespace and the actual published copy. Refusal returns
the actual claimed entry through the held original parent without replacement,
or keeps uncertain data journalled. Platform placement refusal permits no
further filesystem mutation, including a return or cleanup attempt.

A second durable Claimed phase records verified removal intent before unlink.
Verification uses the opened file itself, including final links and raw Unix
names. Available persistent source/parent/stage facts and published copy/parent
facts come from the retained descriptors. A replacement payload after callbacks
is preserved. Normal completion appends the removal and its cleanup atomically.
Cleanup of a returned or removed source keeps only empty-stage rights; it never
retains deletion authority for an original user payload that later reappears.

Returning is a separate durable phase saved before cancellation or verification
refusal returns a claimed entry. It records the actual return subject, including
an unexpected claimed leaf. Recovery can finish that return without replacement
or recognize the proved returned subject. Missing or unknown return subjects stay
journalled; they cannot fall through to Claimed removal. Typed platform placement
refusals still preserve the current claim without attempting a return.

Schema32 reserves these claim phases from older generic cleanup. Startup returns
only a proved original Ready payload without replacement. For a Claimed payload,
it requires original copy/storage identity and both recorded fingerprints before
finishing removal. Changed copies, unknown payloads or storage are preserved for
Recovery. After an already-finished unlink, recovery records only the completed
step and cleanup, preserving newer copied output without replaying it.

Recovery requires the claim's source and copy scalar identities and its normalized
empty quarantine to match the pending removal step. Completed SourceRemoved
records keep available source, source-parent and published-copy original facts
from the held descriptors for future Undo verification. Existing derived Undo
return markers have no fabricated original facts. Undo consumption of these new
facts is separate unfinished work. Matching scalar identities and fingerprints
cannot replace original-generation proof; tests substitute both scalars with
identical replacement objects and require refusal. Inspection errors propagate;
only a proved different original or ENOENT counts as an absent original name.

The private-process matrix kills actual workers before claim, after claim,
after verified intent, after unlink, before return and after return. Two engine
reopens verify preservation of edited copies, unknown payloads and replacement
copy parents, metadata-only recording after completed unlink, and durable return
direction even when the returned source is subsequently missing. The original
two mutation-boundary regressions and typed placement-loss acceptance remain
part of verification; no helper result is counted as a second parent test.

This does not reconstruct a scheduler execution owner. Same-filesystem Move,
empty-folder removal, destination replacement, legacy/unsupported stages, full
step/handoff lineage and the required both-part-orders restart gate remain open.
Trash/Restore, Undo, the full matrix and all later goal areas are also unfinished.
