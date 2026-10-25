# 0109: Keep an interrupted Rename claim in Recovery

Status: scoped backend, Iced and native verification, 2026-10-20. Continues ADR0108 and Phase2;
the full product and durable continuation contract remain unfinished.

An unresolved Rename can contain the original entry or an unexpected replacement
whose no-overwrite return is blocked. Recovery Copy salvages bytes but does not
retire the blocking journal. Add an explicit Keep in Recovery action for an
available ordinary Rename claim. The action describes metadata ownership transfer;
it does not complete or replay the Rename and does not delete or move data.

The selected live Recovery item captures its exact primary journal and observed
payload. A saved request does not retain that review capability. Recheck both
before one immediate transaction inserts a protected Recovery record, its owner
and path indexes, and removes only the matching primary journal/bindings. Refuse
stale/changed/unavailable claims, unsupported journal roles and outstanding folder
permission receipts. Database failure leaves the entire old owner intact. Other
requests, source leases, completed Undo and the Recovery Copy lane remain intact.

Schema25 persists the Recovery record's payload role, previously transient.
Existing records default to the ordinary previous-version role. Claim archives
are protected from expiry and cannot use ordinary saved-container migration.
Readonly directory payloads remain beside their private receipt directory; include
their actual trees in copy destination guards and retained-source protection.
An unexpected sibling symlink retains its final entry, including when dangling;
resolve its parent without following the referent. Refuse moving a claim receipt
separately as an ordinary saved container. Public regressions and preserved
fixtures establish both errors before their fixes (observations09/18).
The same Recovery identity survives owner transfer, but the action is no longer
available after transfer. Copy out remains explicit after restart.

The ordinary failed request remains separate historical evidence and can be
dismissed once its journal no longer blocks review. No completed Rename/Undo is
fabricated. Exact result-save/step reconciliation, durable replay proof and other
operation families still need their remaining implementation and acceptance.

Current attempts and limits: `.scratch/file-manager/CLAIM-REVIEW-WIP.md`.

## Verification and limits

Public core regressions first reproduce two errors in the candidate: moving a
receipt alone makes Recovery point away from its sibling data; following an
unexpected final link loses that entry's source reservation. Both failed datasets
and observations remain. Container ownership now refuses claim receipts before
mutation. Retention resolves the parent and preserves the final entry, including
dangling links, while granting no ownership of its separate referent.

The final library checks pass 37 parents, including actual child SIGKILL after
file/readonly-directory claim, reviewed Keep and later ordinary work. Full backend
passes 612 parents across 67 standard targets plus custom device_runtime;40 ignored
entries are not all independently executed. Default Iced: 105 pass/2 ignored,
formatting, all-target Clippy with warnings denied and the desktop build pass.
All 588 crate/vendor/Cargo inputs remain equal through final documentation.

ELF9cd8f4e26e0d (134829384 bytes) is protected in a decoded-verified gzip. Six
native cases/26 grouped checks pass on that executable and pinned Niri63e39bae3473,
dark at 100% and light at 125%. New claim cases use seeded post-interruption journals with Files
stopped; actual SIGKILL claim proof comes from the core parents. Existing native
Recovery and real interrupted Undo still pass. Four final captures were inspected.

The first light fixture reached successful Keep/copy but requested Browser focus
before the copy destination's modal handoff had settled. The corrected fixture
observes dialog retirement and Recovery focus before one Browser focus request;
both themes pass with before/after snapshots. No application focus or timeout
change, general startup/focus fix, normal-quit proof or kernel tracing is claimed.
All failures and the earlier partial native batch remain reviewable.

This closes ordinary Rename's explicit claim-owner transfer, not Phase2 or the
full brief. Other claim families, exact step/end receipts, filesystem-success and
result-save reconciliation, stable durable originals/media/traversal/stages and
executable restart authority remain open, followed by original handoff phases3–6.
