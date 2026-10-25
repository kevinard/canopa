# 0051: Keep Recovery selections and unfinished work through owner changes

Status: accepted under delegated authority; verified for the scope below.
Broader Recovery and the full file-manager brief remain active.

A cancelled directory Recovery copy must keep its exact remaining work when the
output is temporarily unavailable or another folder occupies its path. Refusing
that new folder protects its bytes, but a plain error also discarded the native
Continue task. A native regression shows the refusal message asking the user to
restore the original folder while offering no way to continue afterward.

Output refusal now produces a paused outcome carrying the same opaque Recovery
continuation. Missing or unreadable captured output folders use the same result.
No new work or Undo entry is reported. Returning the original folder permits
Continue; its captured identity and destination-parent identity remain required.
A new same-name folder does not become authorized output. Fresh occupied copy
locations still fail and require a different name. Source changes still require
Refresh rather than adoption of a newly observed version.

History expiry has a separate selection-lifetime problem. A protected saved
version can move from an active Undo record to an archive when the partial copy
itself creates the 101st entry, or while other operations finish. The saved bytes
are still available, but the old catalog key no longer locates them. Public tests
reproduce both cases, including an identified containing-folder Rename and a
process reopen before continuing.

A Recovery selection now carries its saved-container identity, logical retained
item identity and payload role alongside the exact observed root version. Normal
key lookup validates all of these. If the key has retired, the existing physical
saved-stage owner index streams candidate current owners. A candidate must name
the same saved container, logical item and payload role, and still expose the
same observed version. Filename equality supplies no authority. Mismatched or
changed versions are refused. Unavailable aliases do not prevent inspecting
other indexed candidates, but no caller searches arbitrary folders for a name.
The resolver is shared by native storage preflight and copy execution, so a
captured old path cannot reject an otherwise proven containing-folder movement.

This selection state is in memory. No database format changes or extra pinned
Undo entries are needed; history remains schema 17 with its normal bounded
retention. A completed prefix's older Undo can expire normally while its saved
source remains protected and the unfinished copy stays resumable. Continuing
records new work in a new chronological Undo entry. The observed root check is
not an atomic snapshot of every child or a full original-media descriptor lease.

The native expiry workflow performs 105 real folder creations after a paused
copy, renames the source container through the normal UI and then continues the
captured remaining work. It exposed a second task-lifetime bug: Operations
bounded every non-running row as completed history, discarding a paused task.
The list now retains running jobs and all jobs with Retry, Continue or unanswered
questions, alongside at most 100 finished rows. This retention does not pin or
extend the engine's bounded Undo history. Retry/Resume titles retain one action
prefix instead of accumulating prefixes with every attempt.

Four new public parents cover expiry caused by the copy's own completed prefix,
later expiry followed by containing-folder Rename/reopen, missing or unreadable
output, and queued selection with changed-source refusal. The existing replaced
output test now requires a paused outcome and resumes the returned plan after
restoration. Tests use the public engine and actual private files, with no SQL
output assertions. Native cases create 105 folders through the actual name
prompt and notification path; their preserved continuation resolves the archived
source after an actual containing-folder Rename.

The final source passes 304 core tests with 20 parent-invoked helper definitions,
all-target Clippy with warnings denied, the desktop build and formatting. Native
110/112 basic dark/light and 111/113 expiry dark/light each pass 16/19 checks,
totaling 70 on one executable, with light at 1.25 scale. All 38 captures were
inspected; no GTK warnings, criticals or panics appear in the four application
logs. Light03a catches an early Starting paint, so later04/05 and actual keyboard
continuation provide its paused-state evidence; dark03a directly shows Continue.
The 100 tracked file-manager Rust/build input hashes remain unchanged through
verification. Python syntax, inventory, whitespace and packaged documentation
(24 files, 39 resolving offline links) also pass.

Exact scope, source hashes and red/green evidence are recorded in
`.scratch/file-manager/recovery-continuation-verification.json`; follow-ups are
maintained in `RECOVERY-CONTINUATION-WIP.md`. This resolves protected saved-version
selection across owner-key changes; it does not pin every ordinary unprotected
file selected for a future request. Temporary source-unavailability task retention,
request source ownership/pinning, root-install interruption, general directory
metadata, persisted unfinished jobs, explicit disposal and the full product
acceptance remain open requirements.
