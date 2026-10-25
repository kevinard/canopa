# 0040: Explicit additional-item Merge and owned destination containers

Status: accepted under delegated implementation authority; verified for the scope below.

After a Merge's recorded children finish, its remaining-items review now offers
Merge remaining items alongside Keep remaining items, Skip and Cancel. Skip
remains the Return default. Merge records a new snapshot of the additional work
and processes its children through the existing journalled Undo primitives.
Collisions receive ordinary per-item decisions. Skip, Cancel and reopening keep
the exact unfinished children in the original Undo operation. New contents that
arrive during that pass require another explicit review.

Before recording each snapshot, Files checks the incoming and destination root
identities, both directory metadata versions shown in the review, active
ancestors and cancellation. A stale answer requests a fresh review. This action
does not silently adopt filesystem changes into the old snapshot.

History schema 10 persists a scope for each initial or additional snapshot and
a monotonically increasing child ordinal counter per operation. Nested initial
snapshots inherit their parent's scope and original history boundary. Additional
snapshots have no captured older owners: a reused filename or externally moved
file alone does not establish ownership of already returned work. Their Keep
both decisions cannot rewrite older history of completed children. The counter
prevents completed child identifiers from being reused by subsequent snapshots.

Recovery stages retain their scope for the operation's lifetime, including
after a nested marker completes. A subsequent scope cannot adopt those records.
Within a scope, starting a nested Merge binds its incoming records to that more
specific destination container. Schema 9 migration preserves pending flat and
nested sessions and assigns their initial scopes in parent-before-child order.
Earlier supported schemas continue through the existing migration path.

A pending member's destination is a slot inside a particular merged directory;
it is not ownership of the file currently occupying that slot. A later file
return under a new name leaves that slot intact. Relocating the actual merged
destination directory moves its pending slots and bound Recovery contexts only
after its expected physical directory identity is found at the returned path.
Relocation snapshots these container movements before changing any stored rows,
so results do not depend on database page or parent/child update order.

Incoming member locations follow their owned source containers or a proven
direct return of that exact source. Retained stages follow their actual stage
or parent identity. Trash source relocation must preserve its payload/metadata
pairing. Ordinary older nonmember history retains its established relocation
behavior; active Merge members use these more specific ownership rules.

Public acceptance covers reused child filenames, independent old/new Recovery
versions with copy-out, unrelated later file returns, whole and nested merged
destination moves, repeated nested passes, reopening, old-schema migration, and
stale/cancelled answers. A process-kill test observes the first actual additional
child return before SIGKILL and verifies exact continuation without duplicate
returns. This does not force every root or saved-destination cleanup checkpoint.

Native acceptance extends the existing directory workflow with explicit
additional Merge, stale-answer refusal, a reused-name collision, Skip/retry,
preserved completed work and a Quick Look preview. Final evidence is recorded
in `.scratch/file-manager/merge-additions-verification.json` after validation.

Full directory Merge remains incomplete: cross-filesystem child reversal,
changed/unavailable roots, source-only saved remainders, deeper cleanup kill
checkpoints and broader batch/dependency handling remain work under the active
product goal.

Final validation passes 215 core tests with fifteen parent-invoked child helpers,
all-target Clippy with warnings denied, desktop build, formatting, Python syntax,
maintained inventory and whitespace checks. Native68 dark and69 light at1.25
scale each pass32 checks on the same schema10 binary, with clean GTK logs. Six
new dialog/preview captures were inspected. No native fixture repair was needed.
The final proof preserves the original behavioral failures and corrected legacy
fixture input separately from the final passing results.
