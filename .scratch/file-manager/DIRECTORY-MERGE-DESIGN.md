# Directory return with Merge

Status: first working slice under ADR0038; remainder review and child-install
restart/expiry acceptance under ADR0039. Full Merge acceptance remains pending.
The product goal remains active. Undo Merge is exposed for directory returns.

## Behavior

When an owned incoming directory returns to an occupied directory location,
offer Merge alongside Keep both, Skip and Cancel. Keep both remains the safe
default. Merge reuses the shown destination directory and returns children into
it; it must preserve newer destination work. Each child conflict retains normal
Replace, Keep both, Skip and Cancel decisions. Replacement versions go to
protected Recovery. Links are entries with raw target bytes, never directories
to traverse. Unsupported entries remain pending with an actionable error.

A Merge is part of the original Undo entry. Completed children stay returned
after Skip, Cancel, failure, restart or a later retry. Pending children retain
their source location and ownership. A retry must not copy completed children
again or offer to replace the result of its own completed work. New arrivals
and changed source/destination identities require fresh review. Completion
removes only the owned empty incoming root, then handles the original step's
saved destination or Trash metadata. A saved-only remainder stays retryable.

ADR0039 adds explicit Keep remaining items/Skip/Cancel when contents remain
after the child steps finish. Keep preserves that source folder and protects an
unrestored saved destination in Recovery. It does not relocate old namespaces
onto new arrivals. Additional-item Merge/rescan still needs causal namespace
ownership, particularly when a new source file reuses a returned child's name.

## Interface and ownership

A directory-return module should own this state machine behind begin/resume
operations, hiding child journals, progress, namespace mapping and reconciliation
from the interactive Undo caller. It should reuse the existing transfer I/O and
verified cross-filesystem source removal. Calling public transfer while Undo
holds Engine.state would deadlock; shared execution must accept the already
borrowed store instead of taking the engine lock again.

The implementation must keep four things distinct:

- the original Undo entry and its logical older-history names;
- each physical pending or returned child and its verified identity;
- the existing destination directory, whose newer work is not owned by Undo;
- active and archived retained stages, whose locations can now span both roots.

Reusing a destination root does not make older root-creation history own it.
Only relevant parent references may follow the returned child relationship.
Per-child Keep both may relocate that child's older logical namespace; a later
root-wide relocation must never rewrite already returned siblings. Archived
Recovery follows only actual movements (ADR0037). Active retained stages need
equivalent split-path resolution while the original entry remains pending.

## Persistence decision

Persist a directory-return session linked to the original operation/ordinal,
with source and destination root witnesses and per-child states. Each child
must distinguish preparation, installed destination, verified source cleanup,
and completed return. Changes to disk and session records use durable intents
and existing reconciliation principles. The user sees the original Undo entry,
not internal compensating moves added as new Undo entries.

The working slice uses schema9 DirectoryMerge parent markers and journalled
ordinary child changes in the original operation. Membership guards all
ancestor roots, incoming-archive associations separate logical names from live
locations, and the parent marker owns terminal root removal. Ordinary history
expiry excludes unfinished Merge operations. No hidden operations were needed.
Cross-filesystem child copying and its adapter remain to implement; the current
slice reuses existing rename reversals. See ADR0038 for verified scope and gaps.

## Required proof

Start with a real Rename directory collision, two disjoint child sets and an
unchanged destination root identity. Then test nested Merge, all file-conflict
choices, older Copy/CreatedFolder ownership, Skip/Cancel/reopen/retry, changed
root witnesses, links/special entries and new arrivals. Verify cross-filesystem
copy commit precedes source cleanup, including interrupted cleanup and retries.
Exercise incoming and newer-destination active/archived Recovery through split
returns, expiry and copy-out. Kill real processes at child install, source
cleanup and root cleanup; verify no duplicate data return or silent loss.
Include unavailable previous versions, saved-only continuation, Trash metadata
and removed-source directory ancestry. Native dark/light acceptance must expose
real decisions and truthful partial/completed outcomes. Clean same-filesystem
examples alone do not establish full directory Merge support.
