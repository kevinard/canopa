# 0053: Preserve copied-folder permissions through interruption and Undo

Status: accepted for the recorded permission scope. The full product brief
remains active.

## Context

Private 0700 folders were copied as 0755 under umask 022. Under umask 077, a
read-only 0555 folder became 0700. The wrong mode appeared in ordinary Copy,
cancelled Copy and Recovery from the first progress event. Changing the target
parent's permissions did not change the result: folder creation used defaults
and never applied the source mode.

Preserving a read-only folder also needs deliberate handling of installation,
Continue and Undo. Linux requires owner write access when moving a staged
directory under another parent. Populating or undoing its children requires
temporary write access without making that access permanent.

## Decision

New copies preserve the source folder's normal Unix rwx bits, matching the
existing regular-file copy policy. New folders created by the user continue to
use normal creation defaults. Existing folders selected for Merge keep their own
permissions; a failed write is reported without changing an unrelated folder's
mode. Read-only source contents remain at the source when Move cannot remove
them. Exact Retry can continue after the user restores access.

Copied folders and newly prepared directory-return roots record their intended
final mode before installation. Their private stages have enough owner access
to install them; final permissions are restored before the step is reported as
completed. Recovery distinguishes an uninstalled stage from an installed folder
that moved away. A missing installed object keeps its journal and requires the
original to return rather than adopting a new occupant at the same path.

Root permission authority uses the identity captured from the created stage or
the approved Merge version. Reading a new identity back from the pathname after
installation could otherwise adopt an unrelated replacement. Placement and
physical-root checks occur before traversing children. A refusal after creating
the root records that one completed folder and retains its exact continuation;
Continue reuses the captured root, creation owner and mode. Initial directory
read failures create no empty output or Undo entry.

Temporary access adds only owner rwx bits. Its record is durable before chmod.
Live restoration uses the opened directory object even if that object is renamed
while a copy runs. Restart requires the recorded directory and parent identities
at their original location. A distinct external permission edit is preserved.
An unresolved filesystem journal retains its access record after restoring the
mode, so later reconciliation can safely obtain access again. Primary operations
and Recovery copies keep independent records.

Only captured created-folder parents, newly prepared return parents and exact
directory cleanup owners grant this access. A continuation's copied steps retain
their parent identity even if the original folder-creation history expires.
Temporary mode changes do not create separate user Undo entries. Access is
restored between transfer or Undo steps and on completion/error. Normal history
trimming runs after the last access restoration so these records do not suppress
the existing 100-entry history limit.

The database format is 18. Readers of older formats must refuse it. New optional
journal fields default to absent when reading older history; old records do not
gain permission-changing authority by filename inference.

## Verification and remaining work

Public filesystem tests use isolated child processes with umasks 022 and 077.
They observe actual copied/returned modes, bytes, cancellation, exact Continue,
Undo, Recovery, two-device moves, external permission edits, replaced original
folders, history expiry and process death. New-folder recovery states are
prepared as private journal inputs; all assertions use the public engine and
real files. Raw Unix filenames remain part of the copy cases.

The settled candidate passes22 focused permission tests and326 core test parents
with21 parent-invoked helper definitions. All-target Clippy, the default desktop
build and formatting pass. Native read-only Recovery and ordinary directory Retry
pass90 checks across dark and light1.25 runs, including360-pixel Operations,
Continue, separate Undo, Cut/Paste, unavailable sources and floating previews.
All46 captures were inspected; GTK logs are clean. Exact102 compiled-source
hashes, the binary digest, commands and sessions are recorded in
[the verification record](../../../.scratch/file-manager/directory-permissions-verification.json)
and [implementation ledger](../../../.scratch/file-manager/IMPLEMENTATION.md).

This does not establish atomic directory-descriptor authority for every child
mutation, persistent unfinished jobs, ACL/xattr/ownership preservation or the
broader cleanup/disposal contract. Retained-stage cleanup under read-only parents
still needs its own public tracer. The full file-manager requirements remain
open until their own acceptance evidence is complete.
