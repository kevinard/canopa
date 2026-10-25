# 0037: Keep live Recovery locations separate from returned history

Status: accepted under delegated implementation authority; verified for the scope below.

An older folder can return to its original location while a newer folder already
occupies that name. Keep both returns the older folder under a new name. Recovery
versions physically inside the newer destination must remain associated with
that destination. Versions inside the returning folder must follow that folder.

Historical changes describe logical names, which are relocated when a folder
returns under a different name. Archived Recovery records describe actual live
locations. Applying the same blanket namespace rewrite to both could mark an
intact saved version unavailable. The reproduced case renames a project, creates
a newer project at its former name, protects edited copied data inside that
newer project, then returns the original project with Keep both.

The history transaction now records exactly which archived records followed the
physical namespace movement. Only those records can also adopt the return's
logical Keep both relocation. Newer destination records retain their locations.
Historical change relocation and stable identity ownership keep their existing
rules. Archive indexes update in the same transaction as the records.

A source-only return with an unavailable previous destination keeps a saved-only
remainder. Its archive-follow operation must use the actual returned name before
replacing the history row with that remainder. Otherwise incoming Recovery can
be moved in metadata to the still-occupied original name. The transaction now
uses the relocated previous change for both ownership and physical following.

The set of followed archive IDs exists only inside this transaction. It adds no
persistent representation or migration; history stays schema 8. Interrupted
reconciliation recomputes the same set from its durable actual return intent.

Three public filesystem regressions cover newer-destination Recovery, distinct
incoming and destination versions through reopen/history expiry/copy-out, and a
source-only directory return followed by restoration of its saved remainder.
The native fixture uses ordinary Rename, Copy, Undo, Quick Look and Recovery
copy-out. Final evidence belongs in `directory-recovery-verification.json`.

Directory Merge remains unimplemented. This fix establishes the distinction its
split namespace needs, without claiming Merge or complete directory recovery.
The pending state/interface design is recorded in
`.scratch/file-manager/DIRECTORY-MERGE-DESIGN.md`.

Final core validation passes 193 tests with thirteen parent-invoked child helpers.
All-target Clippy with warnings denied, desktop build, formatting, Python syntax,
maintained inventory and whitespace checks pass. Native62 dark and63 light at
1.25 scale each pass nine checks on the same schema-8 binary, totaling eighteen,
with no GTK warnings, criticals or panics. Four rendered conflict and protected
preview captures were inspected. The initial red regression remains separate
from the final focused, core and native evidence.
