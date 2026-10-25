# 0042: Merge with an unavailable saved previous destination

Status: accepted.

## Decision

Returning the current contents and restoring a saved previous destination are
separate parts of Undo. If the saved version is unavailable and the user chooses
Return moved item only, a folder conflict can still be resolved with Merge.
Merge returns its recorded children, removes only the owned empty incoming
folder, and keeps one saved-restoration step in the same Undo entry. It does not
require access to the missing saved file or its containing folder to finish the
contents return.

If the saved version becomes unavailable during an existing Merge, Undo shows
a dedicated review after returning the recorded children. Skip is the Return
default; Escape cancels. Continue without saved version finishes the contents
return and retains the saved-restoration step. Finish this Merge leaves the
empty incoming folder in place, protects the saved-version record in Recovery,
and finishes the history branch. Counts distinguish actual reversals, items not
returned and saved versions not restored.

Continue and Finish recheck the saved location, its containing folder and the
physical roots of this Merge and its ancestors. Changed observations require a
fresh review. A changed, available saved file or link retains the existing
Restore this version review, with the same root guard during an active Merge.

An approved contents-only Merge keeps that decision across cancellation and
process restart. A saved version reappearing during child returns does not
silently switch it to full restoration. Restoring the remaining saved version
later still reviews its current version and any occupant at its intended path.

## Persistent state and history ownership

History schema 11 adds an optional `defer_previous` field to a directory Merge.
Old markers omit it and keep their existing behavior. Once contents return is
finished, one transaction replaces the marker with its retained-only step,
applies captured parent bindings and clears the directory-return scope and
journal intent. No completed child is recreated.

Schema 11 also indexes pending saved destinations. Both retained-only steps and
live Merge markers with saved data reserve their intended slot independently of
later occupants. The reservation includes the older history references captured
for that saved version. Returning an unrelated later file under a new name must
not move the saved slot or those older references with it. They do follow an
actual move of their containing folder, or the saved version's own restoration
under a new name. Relocation uses captured identities and Merge ancestry rather
than adopting a path because another item currently occupies it.

Migration backfills reservations for existing history. Legacy retained-only
records without a recorded containing-folder identity can still follow a
physically owned saved container. The index belongs to the store's saved
destination module and is maintained alongside the other history references.

## Scope and validation

Public filesystem tests cover both ordinary-file and directory contents-only
returns, active saved-version disappearance, Skip/Cancel, stale answers,
changed saved files, explicit Finish and Recovery copy-out after history expiry,
unrelated aliases, older Copy ownership, actual parent moves, legacy migration,
whole-container disappearance and reappearance. A child process is killed after
an actual returned file is observed; reopening resumes without repeating saved
approval or completed children and can later restore the saved version.

Native acceptance extends the existing directory workflow with both initial and
mid-Merge unavailability, safe defaults, cancellation, stale Continue refusal,
pending restoration and Quick Look of returned and restored contents. Final
validation evidence is recorded in the implementation ledger and
`merge-saved-verification.json`. All 234 core tests pass with sixteen
parent-invoked child helpers; all-target Clippy, formatting, Python syntax,
maintained desktop inventory and the desktop build pass. Native runs72 (dark)
and73 (light at1.25 scale) each pass57 checks on the same schema11 binary. All
ten new dialog and preview captures were inspected; GTK logs are clean.

The full product goal remains active. Cross-filesystem child returns, deeper
physical-cleanup crash checkpoints and the broader Recovery, device, Trash and
integration work remain separate requirements. Changed saved payloads that are
directories or special files still need their own recovery design.
