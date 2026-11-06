# Explicit completion of the Print staging stream

Status: scoped native verified. Print remains opt-in; full portal goal active.

The file setup worker sends four or five O_PATH descriptors and publication
metadata to its owner. The owner waits for the worker, then checks the one-shot
stream without blocking. The stream must contain exactly one descriptor message
and EOF before those handles can be used by later publication workers.

The parallel library test previously failed with WouldBlock at that EOF check.
The isolated test passed. A debugger reproduces the exact error twice by retaining
only the sender descriptor. It confirms the socket has CLOEXEC. A separate probe
through the existing native file client and actual normal worker reproduces the
same staging failure while a forked CLOEXEC copy stays open. Closing that copy
before the worker runs permits all six native file-client checks to pass.

[Fork](https://man7.org/linux/man-pages/man2/fork.2.html) copies descriptors;
[close-on-exec](https://man7.org/linux/man-pages/man2/execve.2.html) removes them
when the new program executes. A copy can therefore retain the write endpoint
during the interval before exec. The exact fork behind the historical failure
was not traced; the controlled probe establishes the mechanism and current
production susceptibility without claiming that historical process identity.

After sending the complete staging bundle, the setup worker now calls
[write shutdown](https://man7.org/linux/man-pages/man2/shutdown.2.html) on that
stream. EOF no longer depends on the final descriptor copy closing. The worker
still reports its publication metadata, and the parent retains its existing
worker-exit, allocation, descriptor-shape and later filesystem checks. Stream
completion does not acknowledge file publication or establish worker retirement.

The receiver continues to reject extra packets and unfinished streams. Unit
fixtures now explicitly complete valid messages and retain a CLOEXEC clone
through receipt. Negative checks require WouldBlock for an open writer and
InvalidData for extra packets or another allocation. No receiver wait, retry or
sleep is added.

The maintained regression uses the existing compiled native file-client harness
and normal worker executable. A Python child retains only a CLOEXEC setup-stream
copy while the actual worker performs setup. The controller owns and retires that
copy. The regression fails on the original worker before this fix. A diagnostic
prototype that explicitly completes the unchanged worker's stream passes all six
file-client checks with five copies still live; that prototype is evidence rather
than a production fallback.

See [the investigation](../../.scratch/xdg-portals/PHASE69-LIVE.md) and
[the original issue](../../.scratch/xdg-portals/issues/07-staging-handoff-test-timing.md).
Formatting, strict workspace Clippy and the original workspace/all-target build
pass. All437 Portal library checks pass with default parallelism, along with
55integration and the PPD runtime check. The original debugger close-retention
reproducer now passes. The actual held-copy regression and closed-copy control
each pass the six native file-client groups; all five held copies remain live
until the successful client finishes. Public frontend/Iced file flows pass three
groups in each appearance, including prepared four-page PDF delivery without a
printer job. All96 observed candidate identities in the native receipts are
terminal and private data is removed. See
[scoped evidence](../../.scratch/xdg-portals/print/staging-completion-evidence.json).

The source change is confined to the setup producer and its unit fixtures. Two
Python scripts retain the actual caller-seam regression using the existing
compiled harness. The old unique harness is losslessly archived once by checksum;
no native executable or dependency graph is added. Full workspace execution is
not repeated for this local Portal change; prior Files failures remain recorded.
Device/Flatpak/deployment, other transport investigations and the six experimental
interfaces remain separate work. Automatic Qt routing stays deferred.
