# New-file and unconfirmed-stage publication crashes

Read [the checkpoint](../../PHASE78-LIVE.md) and
[evidence index](../publication-stage-evidence.json). Reuse the existing Phase74
private Flatpak store, Phase75 SDK, Phase76 A81BE client and Phase77 native assets.
Canopa remains Rust/Iced; the controlled GIO consumer is application-side test code.
Run from the workspace root in the native environment, with no Cargo rebuild,
system installation, routing, queue or physical printer changes.

`native-crash.py --boundary BOUNDARY --destination new --scheme dark --export
--out NEW_DIRECTORY` covers complete, publishing, linked and published for new
files. Setup, partial and copied cover both new and replace in each appearance.
Light selects125%. Each case uses actual public Print, real Iced consent, the
normal service and its actual publisher, then normal discovery/startup recovery.
The same connected app remains alive through restart. New unrelated occupants
must survive; retained objects/original hardlinks must stay unchanged.

`trace-backend.py` extends the Phase77 read-only syscall observer. Setup holds the
initial Ready-record fsync. Partial holds the first real staging pwrite64 exit,
after65536 of80516bytes. Copied holds the complete staged-file fsync before a
completion marker exists. No syscall, worker request/reply or journal is changed.
Only the main backend gets SIGKILL; its ordinary parent-death contract must retire
the held publisher. These are local process-death cases, not power loss or
uninterruptible kernel-wait tests.

The partial source is a valid four-page PDF with comments inserted before xref;
only startxref moves, leaving page objects/streams intact. System qpdf validates
it. The unchanged Other app/client uses an explicit private //crash branch, with
no extra filesystem/network/bus grant. Source80516bytes and actual staged65536byte
prefix are independently checked. Complete small PDF bytes without the payload
marker remain unavailable too. Iced disables copy actions for all unconfirmed
bodies; clicking them must not offer a picker or output.

`prepare-partial.py` is exact failed one-time setup evidence. Its export stopped
before a commit because the new build directory lacked finalization. The already
generated PDF and stage were retained. `finish-partial.py` completed build-finish,
export and install of that same stage, detaching its metadata alias before a
possible rewrite. Receipt01 stays failed; receipt02 passes. Do not rerun either
completed setup transition merely on continuation.

Before setup, one unused private Flatpak GIR XML was retired only after exact
comparison to the retained signed original package. Its receipt retains both
checksums. System data and runtime libraries/notices were untouched. No compiler,
new SDK, package download or dependency graph was used.

The first partial-replace-dark-01 passed separately; `run-matrix.py` is the
completed nineteen-case continuation. All twenty runs/60groups pass. Each run
retains exact probe source and input pins. `check-artifacts.py` verifies the real
partial prefix, empty/full-unconfirmed states and eight exact recovered PDFs/
32pages using explicit system qpdf/Poppler. Their bytes match the known small
source, so prior inspected raster evidence applies without another copy.
`host-audit.py` checks all570 recorded PID/start identities gone and all twenty
private desktop paths removed.

`retire-partial-branch.py` ran after all native cases were terminal. It removed only
the explicit private crash deployment, checked original app branches/documents,
and deduplicated the larger raw source with its staging inode. The local exported
commit remains retained; the original SDK/apps remain runnable. Restore/install
that exact commit only for a deliberate partial-case replay, never as an automatic
setup repeat. `source-guard.py` verifies source/receipt/binary/runtime pins and
affected documentation without repeating native work.

Keep shared Phase74/75/76 allocation below128MiB, each continuation's evidence
below16MiB, combined Phase74–78 unique-file allocation below160MiB and free space
above3GiB. No per-case ELF copies or Cargo cache cleanup. The prior ten Files
workspace failures remain; no full workspace success or Print promotion is claimed.
Next is [actual pending-exit and restart resource investigation](../../issues/11-print-cross-restart-resources.md),
then broader Print/experimental/deployment gates. The eighteen-interface goal
stays active and automatic Qt integration remains deferred.
