# 0030: Scope physical identity expectations to each history reference

Status: accepted implementation decision; verification evidence is tracked in
`.scratch/file-manager/IMPLEMENTATION.md`.

## Problem

A path-wide inode alias cannot represent two historical owners independently.
After explicitly returning a changed moved item, an older entry may legitimately
expect that replacement. A later forward operation can introduce the original
physical item at the same path while saving the previously adopted version.
Returning that new operation must preserve both expectations.

An external move exposes a data-loss case: Copy creates A at q, another program
moves A to p, and independent B appears at q. Moving A from p to q replaces B.
Undo restores B at q, but this does not authorize older Copy Undo to delete B.
Path-wide aliases previously made that deletion possible without review, even
when B merely copied A's bytes and timestamps.

## Decision

History schema 7 stores physical expectations per operation, step and reference
role. Each reference records its normalized byte-preserving path, logical
identity and expected physical identity. Primary roles describe the current item;
parent roles describe the directory needed to recreate a removed source.

Before each forward mutation, its durable intent captures the strictly older
reference keys that own its actual source and, when replacing, its saved previous
destination. The append transaction transfers that context to the history step.
A fresh step starts with the physical identity it actually recorded, independent
of older aliases for that inode or path.

A successful source return updates only the captured source owners. Restoring a
saved destination updates only that destination's owners, including when its
changed version has been explicitly approved. Keep, Finish and saved-only return
do not transfer the incoming item's ownership. Stable reference keys survive
namespace relocation; relocation updates their paths without resetting their
physical expectations. Deleted or expired owners simply cease to participate.

Ownership updates, archive/relocation bookkeeping, history consumption and intent
clearing share one database transaction. A partial source return commits its
source owners together with conversion to the remaining saved-destination step.
The remaining context keeps the destination's owners for the later restoration.

The active Undo entry cannot occur among its strictly older owner keys. Its own
physical expectation therefore remains stable until the transaction consumes or
converts it and clears its intent. Recovery resolves this entry's expectation,
including for Trash and Restore, rather than consulting a mutable global alias.
Reviewed changed moves retain their existing captured version in the Undo intent;
recreated sources retain their staged payload identity. No additional frozen
identity field is needed for ordinary Undo.

## Migration and limits

Versions 1 through 6 migrate their legacy aliases into individual expectations.
Ambiguous foreign aliases on Copy entries require the normal changed-copy review:
old aliases contain no evidence that this Copy entry adopted the other file.
Ordinary Rename and recreated-directory expectations retain supported legacy
resolution. Legacy steps without causal context derive older associations from
their stable migrated expectation. This cannot reconstruct information absent
from old journals; migration favors a new review before deleting a foreign copy.

The old alias tables remain empty compatibility/provenance structures. Live Undo
never reads or writes them. Readers require schema 7 and older binaries refuse it.
Contexts retain the existing per-step storage budget; very large parent-owner
sets still require the catalog-bound work recorded in the implementation ledger.
This decision does not complete cross-filesystem Undo, folder merging, unavailable
saved-version choices, or the rest of the product brief.

## Verification contract

Use public temporary-filesystem operations and public history/recovery results.
Cover unrelated restored destinations, layered adoptions, fresh old-inode moves,
partial returns, Trash/Restore permission failures, process kill/restart, raw paths
and supported migration input. Construct migration inputs before opening the
engine; do not inspect private database outputs. Native checks use the private
keyboard/pointer fixture after the compiled binary is rebuilt.
