# Actual normal-service publication interruption

Run from the workspace root in the native host environment. Reuse Phase74's
private Flatpak installation and unchanged Phase75 runtime; Phase76's Owner/A81BE
app supplies the public Print caller. Read [the checkpoint](../../PHASE77-LIVE.md)
and [evidence index](../publication-crash-evidence.json). No setup/deployment or
Cargo rebuild is required. No system routing, queue or physical printer changes.

`native-crash.py --boundary BOUNDARY --scheme dark --export --out NEW_DIRECTORY`
accepts complete, claimed, publishing, linked and published. Light selects125%.
It imports unchanged Phase75 native assets and the maintained desktop Session,
uses standard frontend277568 and normal470 `--experimental-print`, and gives the
real Flatpak app no broad filesystem/network/bus permission. Accessibility and
Documents-portal setup remain disabled; actual filtered public Print/FD transfer
and authenticated app identity remain in use.

`trace-backend.py` is a test-only Linux x86_64 ptrace parent of the owned normal
backend. It follows only that tree, traces actual Print workers and observes a
successful journal fsync or publication linkat. It reads registers/memory to
identify syscall arguments; it never changes them. It leaves the selected
syscall-exit stop held until a main-backend SIGKILL triggers the ordinary worker
parent-death signal. Every traced task must exit/reap. EXITKILL protects against
tracer failure. Core dumps are disabled.

The first publishing-dark-01 used native-before-export.py and the exact tracer
now retained as trace-before-pidfd.py. That successful run has no export groups.
The tracer's subsequent control uses a pidfd for the main kill and skips a
redundant numeric cleanup signal after reaping. Claimed-dark-02 uses current
sources and both recovery exports. `run-matrix.py` is a completed eight-case
continuation, not a command to rerun. Each case binds the exact native/tracer
source version; the original two cases were not repeated.

All cases obtain real Iced overwrite approval before Print. At the held boundary,
the staged source hash, old hardlink identity, target presence/inode and actual
journal phase are independently checked. The only interrupted service signal is
to the main backend. The public caller gets response2; the stopped publisher's
exit is observed. When the destination is absent, a new unrelated file is created
before normal restart, which must preserve it without restoration or replay.

A fresh normal discovery worker and automatic Iced startup recovery agree on the
retired item. Recovery uses the original app-independent private store, never
reconstructed transaction authority. With --export, fresh Iced Save pickers copy
the retained document and, if present, displaced body. New outputs, original
destination/hardlink and retained object hashes/inodes are independently checked.

`check-artifacts.py` binds ten passed runs/36groups, nine exact-source recovered
PDFs/36pages and seven exact original-file copies. Explicit system qpdf/Poppler
verify page labels and precise A4 boxes. All PDFs are byte-identical to the known
source, so the already inspected Phase76 representative raster applies without
another copy. `host-audit.py` verifies all277 recorded PID/start identities absent
and all ten private directories removed. Both appearances' startup review
screenshots were inspected. `source-guard.py` seals source/receipt/binary/runtime
pins and local documentation links without executing setup or a Cargo command.

Keep the shared Flatpak fixture below128MiB, new Phase77 evidence below16MiB and
free space above3GiB. No new native binary, SDK or dependency graph. All native
cases pass; no failed run is reclassified. Existing workspace Files failures
remain. This covers local reviewed-replacement SIGKILL boundaries only. New-file/
unfinished-copy crashes, power loss, uninterruptible I/O, cross-restart quotas,
Documents/standard-runtime/device/deployment gates and the full18 objective
remain open. Do not repeat completed setup or matrices solely on continuation.

Those remaining new-file/unfinished-copy cases refer to the Phase77 seal.
[Phase78](../../PHASE78-LIVE.md) records the subsequent staging continuation;
keep these original receipts and sources unchanged rather than rerunning them.
