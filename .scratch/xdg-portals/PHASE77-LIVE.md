# Phase77: normal-service death during actual reviewed file publication

Full eighteen-interface goal ACTIVE: eleven packaged, six experimental, Print
opt-in. The user chose Print features first; automatic Qt integration remains
deferred. Previous Phase76 was progress: the real Flatpak token/lifetime seam is
sealed. This phase completes new local reviewed-replacement crash evidence.
All handles are terminal; no build, native fixture, question or approval remains.

Read [the evidence index](print/publication-crash-evidence.json) and
[source-only replay](print/publication-crash-replay/README.md).

## What actually ran

The unchanged normal470 backend runs with `--experimental-print`, standard
frontend277568, real Flatpak Owner/A81BE and actual Iced file/overwrite consent.
The existing app/runtime/store are reused, with no new SDK, C client, executable
copy or Cargo command. A private nested desktop, session bus, host destination
and recovery store isolate every run; no installed routing or physical printing.

A test-only Linux x86_64 ptrace parent seizes its owned normal backend and follows
descendants. Backend threads ordinarily continue without syscall tracing. Only
actual `--print-worker` processes are syscall-traced. The controller stops a
worker at a successful journal fsync or actual linkat return, matching the exact
private destination/journal. It changes no worker request, reply, register,
memory, journal or publication syscall. All filesystem publication is performed
by the actual normal worker. After independently inspecting that held state,
the controller sends SIGKILL only to the main backend. Its real parent-death
contract kills the stopped publisher; the tracer observes/reaps that exit.

Five boundaries pass in dark100 and light125, ten runs total:

| Held boundary | Observed files before death | Restart classification |
| --- | --- | --- |
| Complete | Synced completion marker/Ready record; original target still present | Interrupted; document available, no displaced body |
| Claimed | Synced Claimed record; old inode in previous, target absent | Interrupted; document and displaced body available |
| Publishing | Synced Publishing record before link; target absent | Result unconfirmed; both bodies available |
| Linked | Actual linkat returns0; target is staged inode, journal still Publishing | Result unconfirmed; both bodies available |
| Published | Published record fsync returns0; target is staged inode | Previously published; both bodies available |

Every pending public Print returns response2 on backend loss; it is never reported
successful merely because a local link happened. An existing hardlink retains
the old bytes/inode throughout. When death leaves the target absent, the fixture
creates a new unrelated occupant before restarting. Discovery and normal startup
preserve that occupant's bytes/inode, all retained objects and the old hardlink.
They do not restore, retry, overwrite or clean up the interrupted transaction.

The same actual app and public bus connection remain alive through restart.
A fresh normal worker independently verifies discovery; automatic normal-service
startup opens actual Iced recovery with the same item/status. Both appearances'
Interrupted/Result unconfirmed/Previously published surfaces were inspected.
Recovery exports use fresh Iced picker destinations and leave the source work
and original destination intact. Nine recovered PDFs match the exact 2435-byte
known app input, all36 A4 pages/labels pass system qpdf/Poppler, and seven
displaced-file copies match the original63bytes. One previously inspected source-
identical raster is reused rather than stored/rendered again.

Thirty-six groups pass. The first publishing-dark-01 covers death/discovery only;
claimed-dark-02 adds both exports. Matrix01 continues only the remaining eight
cases, never repeating those first two. All277 recorded host PID/start identities
are absent and all ten private desktop paths removed. No failed native run.

## Source and storage

Normal backend SHA47003b28, Portal unit9cce9bee and integration0444f8ea remain
unchanged, as do Rust/Cargo/vendor sources and CUPS AD09/AEF779/D3b6 components.
Root changes are only the fixture README and Print guide. The guard compares all
1358 prior root inputs against Phase76. Native scripts are retained by checksum;
each run stores its exact probe-source, without per-case executable copies.

The initial tracer source is retained exactly as trace-before-pidfd.py. After its
first successful case, control was tightened to signal the main backend by pidfd
and avoid a redundant numeric-PID cleanup signal after reaping. Later cases use
that current source. No product binary or runtime behavior was patched.

The unchanged shared Phase74/75/76 Flatpak fixture stays below128MiB. New Phase77
source/log/PNG/PDF/receipt evidence is separately bounded to16MiB, with a3GiB free
floor. No new native ELF, binary duplication, target graph, cache purge, package
download or installation. No broad workspace rerun is justified by tooling/docs
changes; Phase73's exact ten Files failures still prevent full workspace green.

Next: new-file publication crash and unfinished/unproven-copy boundaries, using
the same actual service and bounded fixture. Then continue stalled-filesystem/
cross-restart resource policy, Documents exports/standard-runtime apps, device/
driverless/custom precision, deployment/release/routing and the six experimental
interfaces. This phase is local SIGKILL at reviewed-replacement boundaries; it
does not establish power-loss durability, kernel D-state retirement, general
publication crash safety, physical printer acceptance or full goal completion.
Do not repeat these completed matrices or SDK/deployment/workspace steps merely
because a new goal turn starts.
