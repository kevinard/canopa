# 0075: Process-owned selected-source observations

Status: implementation in progress with scoped regressions, 2026-10-18.
Continues ADR0067/0072/0074. The full product brief and execution-authority gates
remain active.

## Evidence and decision

A public OperationJobs regression reproduced an accepted Move consuming an
unrelated replacement at its selected pathname while queued behind a conflict.
Durable scheduling, storage requirements and retained-source leases did not bind
an ordinary selected leaf to its original identity.

The transfer core now owns an opaque ObservedTransfer: an immutable request and
process-local selected-entry observations. Capture it on the metadata worker
before durable admission is acknowledged, and retain it through queued and
running work. Never recapture its sources at dispatch. Restored rows have no
process observation and remain review-only. The Iced frontend adds no source
validation or execution sequencing for this behavior.

Open each unique selected entry with O_PATH, NOFOLLOW and CLOEXEC. Preserve final
symlinks, avoid reading devices or waiting on FIFOs, and hold the opened entry so
its inode cannot be reused during the observation's lifetime. Record ordinary
observation failures per source: a missing entry that later appears must not be
adopted, and one failed source must not block unaffected items.

The engine rechecks observed selected paths and selected-directory ancestors at
its existing source-access checkpoints, including after a conflict answer.
Changed entries are preserved and reported as isolated failures. Missing roots
fail before their first successful check. After verification, an absent original
can also be the engine's deliberate quarantine transition; the existing journal,
payload identity and fingerprint checks still own the removal proof. Absence
alone never authorizes source deletion or replay.

## Resource ownership

Observations share a process-wide budget of at most4096 selected-entry pins and
at most one quarter of the current descriptor soft limit. Clones share their
reservation. A budget refusal rejects the whole admission with an actionable
message; partial captures close before returning their reservations. Exhausted
system descriptor resources also refuse admission rather than accepting a batch
with resource failures disguised as individual missing files. This preserves
headroom for SQLite, native transport and active filesystem work.

[ADR0079](0079-owned-clipboard-replacement-capacity.md) adds bounded temporary
overlap for deliberate replacement of a still-owned clipboard selection. Ordinary
captures retain this budget; replacement retains the old observations and cannot
expand the process allowance beyond half the descriptor limit or8192 pins.

Release process observations when their live owners finish. In-memory engine
continuations retain the original pins under
[ADR0100](0100-live-continuation-selected-sources.md), sharing the same budget
through pauses and failed-item retry. Saved stopped plans remain inspectable
without becoming executable application requests. Their future execution needs
original core-owned authority, not fresh observations of current pathnames.

## Verification and remaining work

The original queued-Move regression failed on the previous production core and
passes with admission-time observations. A second public regression in a private
child with descriptor limit128 reproduced unrestricted admission, then verified
budget refusal, shared reservations, partial-capture release, usable live
questions and later successful operations. Broader core and desktop checks are
tracked separately in `.scratch/file-manager/QUEUED-SOURCE-WIP.md`. Source05
passes447 core summary checks (446 parent tests and one isolated helper),51 Iced
library tests, default Clippy, formatting and a normal desktop build. The full
suite exposed a completed-folder remainder regression: final inspection now
uses original placement and recorded directory identity separately from mutation
checks. Both scoped review axes report no unresolved findings. No new native
acceptance is claimed.

These observations do not freeze file contents, follow relocated originals or
make pathname-based syscalls descriptor-relative. They do not supply request-to-
journal association, durable restart authority, original-media reconciliation or
safe Retry/Continue. Owned Copy/Cut preparation now carries the same observations through Paste
under [ADR0077](0077-core-owned-prepared-clipboard.md), with validation tracked
separately. External selections remain observed at admission. Existing storage
checks and retained-source ownership remain required.

## Queued Rename extension

[ADR0103](0103-queued-rename-observations.md) extends admission-time observations to
Rename's selected entry and both original parent folders. Public regressions
reproduce leaf replacement and parent replacement with a surviving hard link. The
new scope preserves parent aliases, original return, content edits, final symlinks,
raw names and the unchanged descriptor budget. It remains process-local check/use
protection; descriptor-relative mutation and durable object authority are open.
