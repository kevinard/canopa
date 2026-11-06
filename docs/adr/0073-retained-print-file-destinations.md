# Retained Print file destinations

Status: scoped retained-consent/handoff verified; file publication/Print unfinished.

The private Print draft originally returned a selected path. That cannot preserve
the object reviewed during overwrite confirmation: another entry or directory
could occupy the path by the time the backend issues a token. Serialized device,
inode and timestamp values do not keep an inode alive across helper exit.

The embedded Save picker now optionally retains a Print file destination while
preparing its confirmation. It opens the canonical parent directory with O_PATH
and, when present, the existing regular entry with O_PATH/NOFOLLOW. Neither step
creates or opens output for writing. The replacement warning is derived from that
retained inspection, and Replace completes that same inspection. It never refreshes
replacement consent by inspecting a newer occupant. Ordinary FileChooser and
Account keep their existing frontend-owned path-selection behavior.

The Iced draft holds the reviewed objects through its final explicit approval.
Changing the file format or switching to a printer retires the file draft. The
helper sends its parent descriptor and optional existing-entry descriptor through
SCM_RIGHTS on a backend-created private Unix socket. The kernel retains those
objects while the message is queued, including after the Iced process exits.
The JSON reply carries bounded byte-preserving location and version evidence;
it cannot substitute for the descriptors.

The helper duplicates the inherited channel number and restores CLOEXEC before
spawning metadata workers. It does not adopt or close a supplied numeric original.
The backend waits for its own dialog process, then receives the file descriptors
with CLOEXEC and verifies their count, O_PATH flags and object kinds. It checks
the held parent identity, the directory currently named by the displayed path,
the held existing file's identity/version, and the current named entry relative
to the held parent. A new entry cannot turn create consent into replace consent.
Changed data, another inode, a moved/replaced parent, symlinks, special entries and
malformed descriptor messages require fresh selection. The parent adapter owns
the resulting non-serializable Pinned value for the future prepared-token payload.
The socket is one-shot: after its own helper exits and the local copy of the
child endpoint is closed, the parent rejects any second message or trailing data.

This implements retained consent, not file publication. Verification followed by
an unconditional rename is not a conditional replacement transaction. Publication
still needs its own staged document, create-without-replacement path, reviewed
replacement disposition, cancellation/ownership and crash/recovery behavior.
No public Print interface or descriptor advertisement is added here. The exact
Print methods, original-client/token/Lockdown integration, toolkit transform/copy
compatibility and printing crash policy remain required by ADR0070/0071.

Boundary tests cover producer exit without writes, late appearance, data/inode
replacement, changed directories, symlink/special-file refusal, exact non-UTF-8
names and invalid handoffs. An existing compiled-harness native fixture drives the
production Rust parent adapter and actual Iced helper; it avoids building another
large standalone Rust test binary. Current validation status is authoritative in
`.scratch/xdg-portals/CONTINUE.md` and `target/portal-print-32/`.

The final source passes formatting, strict whole-workspace/all-target Clippy,
the original graph all-target build,54 Print tests (one native fixture ignored in
ordinary runs), and full Portal regressions (270 library/9 ignored,55 integration/
2 ignored). Twenty-eight scoped native groups pass in dark100/light125 with20
Rust parent reviews and20 Iced helpers. Four actual file pins are received and
verified, two late appearances are refused, and reviewed existing bytes remain
unchanged. All85 observed processes are terminal; source/binary pins are unchanged.
No jobs are submitted or application outputs created/truncated. See
`.scratch/xdg-portals/print/file-authority-evidence.json`. This is not public Print
or file publication acceptance; the earlier ten Files workspace failures remain.
