# 0113: Record exact stopped transfer results before application checkpoints

Status: accepted for result preservation, 2026-10-22. Continues ADR0111 and ADR0112.
This preserves returned results; restart execution remains unfinished.

Copy/Move can return a completed prefix, failed entries and remaining work,
then die before the application saves that result. An execution-start record
and filesystem journal do not reconstruct the exact returned split or its
chosen Keep Both targets. Replaying the initial selection would repeat work.

The filesystem core owns the serializable transfer report and stopped-result
format. Existing application API paths and serialized fields remain compatible.
A normal engine return after finalization constructs the exact stopped report
and descriptive continuations. Schema29 stores a separate typed stopped-result
receipt against the accepted Running revision/body and transfer intent. It also
retains hashes of the original report and each exact returned part. Completed
and stopped receipts cannot claim the same execution end. Fatal or uncertain
engine errors do not establish a stopped receipt. Cancellation before an
execution reservation remains outside this receipt path.

Startup can save the proven stopped result as application metadata, preserving
its original storage requirements, completed prefix and Undo association. It
performs no transfer filesystem work. Missing, legacy, changed, stale or invalid
receipts leave the original request reviewable. New-process clipboard intent
and process execution owners are not reconstructed from these records.

An unsaved live end retains an engine-scoped witness and the actual result.
Retry Save persists metadata only and retains the existing activity, dispatch,
shutdown and source-retention barriers until success. The witness checks its
engine, exact request and unchanged stopped result. It cannot be serialized or
used as filesystem execution ownership.

Receipt bodies use the existing 4 MiB request allowance, rather than the
128 KiB journal-step limit. They count toward the existing per-request and
64 MiB total budgets. Saving the application result atomically retires the
duplicate full receipt and records the saved revision, phase and body checksum.
A live witness may perform one atomic receipt/result write if storing both
full copies would exceed the request budget. A failed transaction retains the
original state; no storage limit is raised.

These checks establish the exact historical result, not restart mutation
permission. Durable traversal, stage and cleanup proof and descriptor-relative
operations on validated original objects remain required before restoring
Retry/Continue. Handoff and abandonment also need typed lineage updates before
later saved revisions can supply such authority. Generic metadata edits cannot
supply that proof. The unignored public Copy/Move restart gate remains required,
as do the other operation families and the full product/release gates.

Verification, 2026-10-22:139 selected regression parents across8 targets pass,
with7 ignored inventory entries. The isolated default-feature library passes115
parents with2 ignored. Default all-target Clippy and formatting pass on834
unchanged build inputs. The required unignored killed-process Retry/Continue
gate is run separately and still fails because startup creates no execution
owner. Its retained schema29 fixture now proves the stopped receipt and all
report/part/saved-body checksums, preserving the newer completed-prefix edits.
No current-source native, workspace or release acceptance is claimed.
