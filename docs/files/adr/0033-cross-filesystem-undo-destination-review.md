# 0033: Review changed and unavailable cross-filesystem move destinations

Status: accepted under delegated implementation authority; verified for the scope below.

A move across filesystems records a destination copy followed immediately by its
source removal. Undo must return a verifiable destination version before removing
that copy. Changed or unavailable copies now receive their own review, with Skip
as the Return-key default and Escape cancelling the remaining Undo.

For a changed file or link, Return this version selects the shown current version
for return to the original location. Keep item leaves it at the destination,
finishes both move steps, preserves any saved previous destination in protected
Recovery, and keeps created destination containers. Folders and special nodes can
be kept or skipped; they are not opened as copy streams. Links retain their raw
Unix target bytes. Existing original-location conflict choices still apply after
selecting a version to return.

The selected physical identity, metadata version and fingerprint travel in the
durable SourceRemoved Undo intent. The selected fingerprint also supplies returned
metadata. It must still match before and after preparation and immediately before
installation. Once the return succeeds, the transaction that consumes the source
step updates only the paired copy's expected identity and cleanup fingerprint.
Later edits at the destination need a fresh decision. Older Copy Undo retains its
original content fingerprint and must review edits returned by this newer move.

Pairing uses the exact preceding ordinal in the same operation, and validates
both the recorded destination path and logical copy identity. It does not search
for other history rows sharing a path or an identity. Missing legacy pairs confer
no cleanup authority. History schema 8 admits the new optional reviewed version;
v7 forward records omit it and upgrade without changing their ownership context.
Older readers refuse the newer schema rather than discarding durable review data.

For an unavailable destination, Skip and Cancel preserve both steps. Finish this
step leaves current files untouched, consumes both steps atomically, and protects
any saved previous destination in Recovery. It reports the incoming item as not
returned and any saved predecessor as not restored. Before finishing, both the
lookup/read failure and any observable metadata version must still match what
was reviewed, including for an unreadable regular file.

When the destination is definitely missing and an owned saved file or link is
available, Restore previous destination restores only that saved version. It
does not return the incoming item or acquire ownership of another file at the
original source path. The saved stage and shown version are checked again. The
actual RetainedOnly intent retains the source step's key until filesystem
restoration succeeds; live completion and restart then apply the paired copy's
destination ownership and consume both rows in one transaction. A process kill
between filesystem restoration and history commit remains replayable.

Within a batch, kept and dismissed destinations join skipped paths as barriers
to earlier frames that would overwrite them or move an ancestor away. This does
not settle the remaining broader dependency policy across partial retries.

Public verification uses actual separate filesystems through transfer, Undo and
Recovery. It covers edits and replacements, Keep/Finish, saved-only restoration,
changed review inputs, later cleanup edits, raw links, folders/FIFOs, v7 upgrade,
older Copy protection, and real process kill/restart during 100-item reviewed
returns and saved-only restorations. Native dark/light acceptance uses normal
cut/paste, dialogs, Quick Look and Recovery. Exact results are recorded in
`cross-current-verification.json` and the implementation ledger.

Directory conflicts/Merge, phase-specific interrupted recovery, full dependency
choices, original-media/directory-descriptor authority and the rest of the product
brief remain open. This decision does not declare Undo or the product complete.

Final verification passes 176 core tests, with thirteen child helpers exercised
by their parents. Build and all-target Clippy with warnings denied pass. Native
52 dark and 53 light at 1.25 scale each pass seventeen checks; original-location
regression 54 passes twelve, totaling 46 on the same schema-8 binary. All use
actual device IDs 35/2050, remove the private secondary directory after shutdown,
and have clean GTK logs. Three rendered changed/missing reviews were inspected.
Formatting, Python syntax, maintained inventory and whitespace checks pass. The
first Clippy attempt found a zero-valued permission literal in a test; changing
it to equivalent octal 0o0 fixed the lint. No application change was needed.
