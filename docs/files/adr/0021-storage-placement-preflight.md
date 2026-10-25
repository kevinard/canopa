# Storage placement follows queued request intent

Status: accepted under delegated implementation authority; implementation and
acceptance in progress, 2026-10-16.

A path that was on a mounted device may still exist after unmount, exposing an
ordinary backing directory. Files must not treat that directory as the intended
destination of an already requested copy. The same rule applies when a browsing
alias changes or another device reuses a mount point. This extends ADR 0020; it
does not declare removable-drive or in-flight mutation safety complete.

The desktop storage module remembers known roots in private `volumes.json` state,
using original Unix path bytes, a canonical placement and backend device identity.
The file is versioned, bounded to 1 MiB and 1,024 roots, and written atomically
through the existing private state writer. Loaded roots start unavailable. A live
mount observation must prepare its canonical path and kernel identity on a worker
and durably save the placement before admitting writes. Read or validation errors
fail closed. No private operation database is queried by acceptance tests.

GIO mount removal immediately invalidates the matching observation epoch, before
releasing the device-operation queue barrier. Stale removal/preparation results
cannot invalidate or admit a newer observation. The UI does not canonicalize paths
or access filesystem metadata. Worker preflight follows directory aliases, retains
missing suffixes and preserves final links when the operation addresses the link
itself. It checks the request's captured epochs and the currently observed device,
plus the root's device/inode and Linux mount ID.

Directory snapshots now include the resolved location observed during their
existing worker read, alongside the original user-facing path. A queued job binds
its originating browsing location to that observation. Changing the alias while
the job waits therefore refuses the job instead of redirecting its destination.

Preparation examines transfers and continuations, create/rename, Trash/restore,
Recovery copies, custom-action working directories/selections, Undo targets and
interrupted-journal targets. The operation engine supplies read-only Undo target
information through its normal row decoder. This keeps schema ownership with the
engine and storage policy with the desktop adapter.

A refused preparation creates an Operations entry with an attention explanation
and explicit Retry; no engine mutation or custom command was started. Retry and
Continue retain the original device identity and browsing binding while obtaining
a fresh availability epoch. They must not silently authorize replacement media at
the same path. A user deliberately choosing another location starts a new request.
When GIO provides no nonempty UUID, the location URI identifies a placement but
cannot establish that remounted media is the original device. Retry therefore
requires the original mount epoch in that case. A fresh reviewed request may use
the currently mounted location. This deliberately avoids guessing media identity
from a path or from filesystem IDs that can be reused after unmount.
Repeated retries retain one original requirement map rather than accumulating
recursive snapshots or repeated title prefixes.

Native acceptance uses the existing public GIO/provider and file-operation seams
with real isolated files. It reproduced backing-directory writes, queued alias
redirection and a retry accepting replacement media before their fixes. The
workflow also exercises successful Retry when the original device returns and
clean restart with the device unmounted. Its private provider deliberately keeps
the backing directory present and can change the device UUID independently of
the path. Exact completed evidence remains in the implementation ledger.

The final queued-request workflows pass in `native-device-safety-08` (dark),
`native-device-safety-09` and `native-device-safety-12` (light, 1.25×): five checks
cover the backing directory, replacement media, original-device Retry, alias
retargeting and restart. `native-device-identityless-11` passes the no-UUID
refusal and fresh-request cases. Their Files logs are clean. The existing
27-check desktop regression passes in `native-acceptance-29`; the stale
notification Undo check caught a classification regression during development.
Ordinary task-validation failures retain their existing Failed status; only a
storage refusal gets the unstarted request's attention explanation and Retry.

Startup history reconciliation uses the same placement check. The operation
engine accepts an optional caller-supplied recovery check through
`open_with_recovery_check`; its ordinary `open` preserves the established
standalone behavior. The engine decodes both interrupted journal lanes and offers
their paths before replaying either one. Refusal leaves the engine inspectable,
preserves both journals and their files, and exposes the reason through the
existing interrupted-operation interface. It does not turn a transient offline
device into a permanently unusable history store. Devices publish their initial
observations before history initialization starts; filesystem preparation remains
on workers. If preparation is still pending, startup conservatively leaves
reconciliation for explicit Retry in Recovery. Subsequent desktop Retry requests
use normal queued-job placement checks. This check covers startup placement,
not the still-open association between persisted history and original media.

The core crash test first reproduced startup deleting the actual unfinished
recovery copy despite a refused placement check, then passed with both journal
lanes and bytes preserved. Native `native-device-recovery-red-14` independently
reproduced startup changing the backing folder with the volume offline.
`native-device-recovery-15` (dark) and `native-device-recovery-16` (light, 1.25×)
pass all three guarded-startup, remount/Retry and subsequent Undo checks. Their
Files logs are clean, and rendered inspection confirms the offline explanation,
Retry control and cleared Recovery state after reconciliation.
`native-interrupted-storage-30` passes the existing eight
recovery checks at light/1.25×, including original-name Quick Look, copy-out,
second restart, 360-pixel controls and separate Undo. The full core suite remains
97 passing tests with eight parent-invoked crash helpers ignored at top level;
native build and all-target Clippy also pass.

This is request preparation. [ADR 0022](0022-live-transfer-placement.md) adds
transfer checkpoints during copying, conflicts, commits and source removal,
with per-source failure isolation. Remaining work includes complete filesystem
authority after a worker starts, persisted Undo/device
associations, clipboard/drag-origin and chooser bindings, directory-instance
replacement and pinned-descriptor race handling, and an unavailable browsing and
preview state. Non-transfer operations still share one preparation decision;
their in-flight checks remain open. Kernel/physical unplug tests remain distinct
from the provider fixture. These requirements remain open before claiming complete
device-loss safety.
