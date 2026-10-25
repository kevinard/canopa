# 0111: Restore exact completed Copy/Move results after restart

Status: scoped backend, desktop and native verification, 2026-10-21.
Continues ADR0110; executable continuation of unfinished transfers remains open.

Copy/Move can finish their filesystem work and die before the application result
checkpoint commits. A request-to-operation association and confirmed changes alone
cannot distinguish that case from an interrupted traversal or cleanup. Replaying
its selected paths would overwrite later work or adopt a replacement source.

Schema27 adds a separate typed transfer completion record, associated with the
exact accepted Running revision, body digest and filesystem operation. The start
transaction reserves the transfer kind and byte-preserving destination intent
before mutation. A successful live engine return after directory access cleanup
and history trimming may record a terminal result: operation, completed count,
bytes, skipped paths and actual destinations. Cancelled, failed, partial,
unavailable, unfinished-source-folder and uncertain results do not establish it.
Recovery copies and direct transfers without an application request do not gain
an application completion record.

Both the reservation and complete result count against the existing per-request
4 MiB and total 64 MiB metadata budgets. The result has variable size; reserving
the intent does not guarantee its later result will fit. If saving it fails, the
actual successful outcome and an unserializable, engine-scoped witness remain
held for metadata-only Retry save. Until saving succeeds, dispatch, shutdown and
removal protection follow the existing unsaved-result rules. Death before an end
record commits leaves reviewable interrupted work, even if its files look done.
No success is inferred from an empty journal, a pathname, or fresh availability.

On startup, the backend accepts only an exact current request and a validated
intent/result pair. It restores the complete historical transfer report without
reading or modifying current source or destination files. The independent result
and source-release transaction validates the same receipt again, then retires its
duplicate end report atomically with the saved result. Large finished results
keep the existing payload allowance. A save failure
retains the restored report and activity hold for Retry save. History expiry does
not erase the receipt or recreate expired Undo. Old histories gain no inferred
ends; missing, unended, stale or corrupt evidence remains review-only. Current
clipboard/Cut ownership is never rebuilt from an old completion.

This closes one completion boundary. Durable original source/device/traversal
proof, safe replay of remaining steps, Trash/Restore, Undo and interrupted cleanup
still need their own implementation and crash acceptance. The broader product,
workspace/native acceptance and release gates remain open.


Verification:162 backend parents pass across the library and seven selected
queue/transfer/startup/source-retention integration targets. The default desktop
library passes107 tests (2 ignored); all-target Clippy with warnings denied,
formatting and the default build pass. All830 recorded Cargo/crate/vendor inputs
remain unchanged. This is scoped coverage, not the full workspace suite.

Six distinct native cases pass16 checks across dark/100% and light/125% on
ELF b379372fb3f1 and the pinned Niri compatibility executable. Runs02/03 cover
live Retry/Continue, ordinary mutation completion and new Copy/Move process-death
restoration. Four final Copy/Move captures were inspected. Run01 was blocked at
private-bus socket creation by the sandbox; run02's light transfer case stopped
at a5-second screenshot timeout after Copy restoration assertions. Run03 repeats
both transfer themes successfully without application, fixture or timeout changes.
The failed attempts remain recorded; this does not establish a reliability fix.

The real cross-filesystem crash test also preserves a replaced source entry and
replaced destination root after a completed directory Move. Corrupt, unended,
stale, legacy and partial work gains no replay authority. End/result-save refusal,
Undo expiry and metadata-budget ownership transfer have explicit regressions.
See `.scratch/file-manager/TRANSFER-COMPLETION-WIP.md` and the verification receipt.
