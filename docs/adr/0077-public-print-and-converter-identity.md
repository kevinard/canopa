# Public Print integration and converter identity

Status: scoped public/native acceptance passed. Print remains unregistered in the normal
service and unadvertised in shipped metadata. This adds native acceptance seams
without changing installed routing, services or queues. The broader eleven
packaged / six experimental / Print roadmap remains active.

The real controller and Production adapter can be hosted by an ignored fixture
in the existing Portal test harness. It requires an explicit fixture root and
checks that the session bus socket is under that root. It registers Print and
Lockdown, starts the real policy relay, and supports graceful test-owned shutdown.
Production still selects the normal backend executable for Iced and document
workers. No fake approval or fake renderer is used along this path.

A native driver uses the pinned standard frontend, stored nested Niri, existing
Wayland parent/pointer helpers, and the application-side GTK probe. An empty
private CUPS scheduler supplies discovery for file cases. A separate loopback
IPP receiver and fixture-only queue exercise printer delivery. The receiver uses
its own discovery bus even with service advertisement disabled. None of these
components use installed portal routing or configure installed printers.

Initial actual public tests exposed a copy-count inconsistency. File selection
correctly retired printer-specific device choices, but reset JobOptions.copies
to1 while retaining the user's layout copy count. The controller rejected the
approval. File grants now preserve that approved count and clear only printer
options. The existing Iced approval case also checks device/layout copy equality.
Copies are still rendered once, according to Prepared/Direct provenance; this
fix does not relax the controller's consistency check.

A converter must execute the same backend inode as its owning worker. Opening
/proc/self/exe pins that inode even after an upgrade has unlinked its path.
The layout helper is executed through its inherited /proc/self/fd/N magic link,
inside the same isolated namespace. The system runtime and private proc are
available there. The executable is read-only, and no binary is copied to a new
artifact or temporary filesystem.

Holding the inode alone is insufficient for bubblewrap's bind-fd setup. The
installed0.13.1 fails when binding an unlinked held file. Its inspected
[source](https://github.com/containers/bubblewrap/blob/main/bubblewrap.c)
explains that bind-fd resolves the magic link before mounting and checks identity
afterward because direct cross-user-namespace FD mounting is unavailable. A small
owned-file probe records the actual failure; a direct FD-exec probe runs the
layout helper successfully from an unlinked executable. A stronger worker-level
replacement case is part of native acceptance below.

Before each converter exec, close_range with CLOSE_RANGE_CLOEXEC marks all fd3+
close-on-exec. Only the explicitly held executable descriptor is then retained
when needed. Stdio carries the approved input, fresh bounded output and diagnostic
pipe. Thus raw IPC descriptors or an accidentally inheritable cancellation writer
cannot enter the converter. The existing address-space, CPU, file-size, temporary
filesystem and diagnostic limits remain. This requires the Linux close_range
CLOEXEC facility; unavailability fails closed and remains part of runtime packaging.

The namespace lifetime fixture forks a child which calls setsid and then forks a
grandchild. It checks ordinary namespace-PID1 exit, cancellation and loss of only
the owning harness. The parent-death/namespace lifetime must retire detached
children too. A document fixture also attempts host-path read/write access and
checks owned canary bytes and publication behavior. Exact source/pins and PIDFD
retirement evidence are recorded per run; none of these checks is a claim about
all hostile documents, physical printers, full backend recovery or power loss.

Remaining gates include duplex/collation boundaries, other toolkit prepared
formats, Qt, recovery/ambiguous CUPS creation and stalled filesystem setup,
advanced device options and runtime distribution. Shipped Print registration
still requires the complete applicable acceptance. The ten recorded Files
workspace failures, six experimental interfaces and final release gates remain.

Current acceptance uses normal backend cd2152ad… and the exact Cargo-selected
harnesses. Format, strict workspace/all-target Clippy111.04s and the original
graph all-target build383.15s pass. All89 Print/7ignored and12 Lockdown cases
pass; complete Portal regressions pass307 library/15ignored and55 integration/
2ignored. Guarded source and executable pins remain unchanged through acceptance.

Sixteen public groups pass in dark100/light125: cancel, Close, original-caller
loss, new and reviewed replacement files, prepared-token handoff with unchanged
already imposed/copied bytes, a live GTK3 application through the actual standard
frontend/Iced/file path, and actual private CUPS/loopback IPP delivery. Both
printer jobs receive the exact7,353-byte prepared document. The scheduler's actual
Get-Job-Attributes reply confirms copies1 and the receiver observes one-sided/A4
media. Its environment omits default copies, so the test queries the scheduler
rather than assuming an absent attribute proves a value. All44 observed public
processes are terminal, with no cleanup kills. The dialogs were visually inspected.

Five native lifetime/identity groups pass with23 observed processes terminal:
namespace-PID1 exit, cancellation, loss of only the owning harness while children
changed session/process group, actual replacement/unlink of the executing
worker's temporary hardlink, and a PostScript host-path read/write canary. The
worker continues rendering through the held executable inode; canary reads and
writes are denied. Twenty-four existing production file/conversion/storage/loss
groups also pass on the corrected runtime, with130 observed processes terminal.
These are scoped tests, not a proof against every document exploit or full
backend/power-loss scenario.

Earlier failures remain recorded: inconsistent file-grant copies; an inherited
fixture pipe writer before boundary hardening; pathname-based bind-fd after
unlink; missing private discovery bus in the printer fixture; an assertion that
expected default copies in the receiver environment; and a click driver which
assumed fixed logical dimensions at125%. Corrected fixtures use actual Niri
logical dimensions and explicit scheduler job attributes. No tests were disabled.
The real copy and descriptor/identity defects were fixed in production code.

Authoritative receipts are `target/portal-print-36/acceptance-01.json` and
`.scratch/xdg-portals/print/public-evidence.json`. Existing compositor/frontend/
CUPS/application probes and the same root incremental cache were reused. No standalone
Rust driver or additional fixture executable was built or copied; temporary executable hardlinks
were removed. Net phase growth at sealing is34,402,304 bytes, with about7.42GB
free. No new full-workspace test gate ran; the ten recorded Files failures and
all broader activation/distribution requirements remain open.
