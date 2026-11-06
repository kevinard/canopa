# Staged Print file publication

Status: scoped private publication verified. Public Print remains
unfinished and unadvertised. This follows the retained consent in ADR0073.

The Print worker can now copy an approved sealed document to a private ordinary
file in the selected directory. It copies from offset zero with pread/pwrite,
without changing shared offsets, and requires the approved PDF, PostScript or
SVG format. The bounded sealed source is the same immutable document used by the
printer worker. The destination is never opened for truncation or writing.

A `Publication` consumes one retained `Pinned` consent. The independently owned
client task retains the sealed document, directory, optional original entry,
staging directory, document and journal handles until publishing and settlement
helpers have exited. Filesystem setup runs on a blocking thread; copying and
commit run in the isolated worker. Dropping the requesting future signals the
worker's cancellation pipe, then a separate uncancelled helper settles the work.
The operation permit is released only after that settlement attempt. A failed
settlement reports and retains recovery instead of recursively deleting work.

The work directory has a random name and mode0700; the staged document and journal
are mode0600. The journal records byte-preserving selection evidence and held
object identities, followed by bounded synced state records. The directory and
its parent are synced before displacement. Neither a journal nor serialized
device/inode values are authority to reopen or replay a transaction after a
backend restart. Live descriptors remain necessary for automatic settlement.

For a new file, `linkat` publishes the actual staged descriptor through
`/proc/self/fd/N` with `AT_SYMLINK_FOLLOW`. It cannot replace an existing name.
This avoids a source-name substitution and the capability required by
`AT_EMPTY_PATH`. See the primary [linkat contract](https://man7.org/linux/man-pages/man2/link.2.html).
The present implementation requires procfs and same-filesystem hard-link support;
an unsupported filesystem fails with the staged bytes retained if the outcome
cannot be established. A rename-based filesystem fallback is not implemented.

For replacement, the worker revalidates the held consent, records Claiming,
and moves the current entry to `previous` using `RENAME_NOREPLACE`. It verifies
that the displaced object is the held original, allowing the ctime change caused
by rename while retaining the other reviewed version facts. A different entry
caught between verification and claim is restored without replacing another
name. If restoration is blocked, that entry remains in the private directory.
No unreviewed displaced entry is unlinked. New output uses mode0600 and leaves
existing hardlinks and open handles to the old file unchanged.

The worker records Publishing before linking. A syscall error does not reset
that state: NFS may perform a link or rename and lose its reply, as documented by
[link(2)](https://man7.org/linux/man-pages/man2/link.2.html) and
[rename(2)](https://man7.org/linux/man-pages/man2/rename.2.html). A positive worker
receipt, a valid Published record, or the selected name matching the retained
payload establishes publication. Settlement records reconciled publication before
cleanup. A missing or different target after an unacknowledged Publishing state
is uncertain; it preserves both the staged bytes and displaced original and
does not restore or retry automatically. A partial journal is conservative too.

Before the Publishing boundary, cancellation or worker death can restore the
actual saved entry using no-replace rename. After confirmed publication, cleanup
unlinks a saved original only if its recorded claimed version still matches the
held review. Changed originals, foreign children, altered private entries and
cleanup failures preserve recovery. Cleanup removes only known entries and can
resume across its own unlink operations and completed-directory removal. The
private0700 namespace is owned by these workers; it does not defend against a
hostile process with the same UID or ptrace rights.

Regression coverage includes complete new output, reviewed replacement with an
old hardlink, late files/symlinks, moved parents, changed originals, cancellation,
unreviewed claim restoration, conflicts, corrupt journals, lost commit replies,
idempotent cleanup and actual SIGKILL at transaction boundaries. The native client
fixture uses the normal workspace executable for publication/settlement and a
controlled worker boundary for caller-future loss. Both fixtures reuse the normal
compiled library harness; no extra large Rust driver or binary copies are made.
Formatting, strict whole-workspace/all-target Clippy and the original all-target
workspace build pass. All69 Print tests pass (three fixtures ignored in ordinary
runs); complete Portal regressions pass285 library/11ignored and55 integration/
2ignored. Six normal-client groups pass with nine observed worker processes all
terminal. Two real partial-write EFBIG cases use RLIMIT_FSIZE at16KiB, preserve
new/existing destinations, preserve the shared source offset and remove the
incomplete stage through production settlement. This is not an ENOSPC or remote
filesystem acceptance result. No new full workspace test gate ran; the preceding
ten Files integration failures remain unresolved. Exact source/binary receipts
and limits are in `target/portal-print-33/acceptance-01.json` and
`.scratch/xdg-portals/print/publication-evidence.json`. Phase net storage growth is
about217MB, with no new Rust native driver or copied application binaries.

This does not complete Print. Public methods, request/original-client/token and
Lockdown integration, toolkit layout/copy rendering, device-control compatibility,
startup recovery presentation, abrupt backend/power-loss acceptance and the
CUPS ambiguous-creation policy remain. Setup metadata currently uses a blocking
thread; isolating setup/recovery for indefinitely stalled filesystems remains a
gate before advertising Print. No installed routing, service or print queues are
changed. The broader eighteen-interface goal remains active.
