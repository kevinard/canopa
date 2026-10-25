# 0091: Backend-owned device catalog and commands

Status: catalog, shared admission, parent mount/unmount, current-root lookup and
backend attention scoped-verified;
full implementation in progress,
2026-10-19. Continues ADR0020/0074/0081.
The entire removable-drive requirement remains open.

Extend the existing process-owned GIO helper, rather than starting another
volume monitor in Iced. The runtime owns the device catalog, explicit mount and
removal requests, cancellation, credential questions and their lifetimes. Iced
will render snapshots and send intent bound to a current observation. A command
may outlive its originating window; only successful-mount navigation belongs to
that window and must still match its Tab/navigation intent.

LocalOperations publishes a prepared catalog and a separate renderer-independent
DeviceRequests interface. Commands require explicit invocation; ordinary startup
only observes. Backend attention is described below; Iced presentation and
scoped acceptance are recorded in ADR0093.
Device operations and removal coordination belong outside the renderer.

Catalog entries are local GIO volumes and standalone local, unshadowed mounts.
An associated volume and mount appear once. Unmounted volumes remain visible.
Display names and UUIDs are not request targets. Opaque tokens combine helper
object identity with the current observation generation, expire on the next
platform observation and are never persisted. A token is neither filesystem
execution authority nor permission to remove a device while Files work exists.
Command admission revalidates it in the owning runtime and again in the helper.

Wire version2 adds device names, mounted-root references and capabilities to the
existing bounded helper frame. The pinned helper and parent always run from the
same executable; this is an internal protocol, not stored state. Validate IDs,
generation monotonicity, duplicate device/root associations and size bounds.
Roots retain Unix bytes and reference the frame's existing mount observations.
The parent publishes a Ready catalog only after all Storage preparations finish.
Failure or orderly stop clears current entries. One bounded coalesced notice
stream covers catalog changes and failure; consumers never drive preparation.
Volume and drive signals must update the catalog even with no mounted root.

Capability selection prefers safe drive stop, then drive/volume/mount eject,
then unmount. Commands use asynchronous GIO operations with no force flag,
preserve exact device identity throughout the request, and support
credentials/cancellation.
Removal must be serialized with both the filesystem queue and arbitrary custom
actions in the backend. A new Files job during removal waits; checking only a
frontend's cached busy state would race admission. Device work never enters Undo.

The application now composes one LocalActivity across filesystem work and custom
actions. Its removal guard is conservative and global because an arbitrary
script can access paths beyond its selected files. Work reserves a hold before
entering a control channel, then retains it through durable admission, queued
execution, questions, cancellation teardown and unsaved results. Execution workers
share the hold so coordinator disposal cannot release it before actual teardown.
Save retries reuse their existing job hold; stopped historical records grant no
new execution authority. Removal admission is synchronization only: the device
workflow separately validates identity, capabilities and user intent.

Work arriving during removal waits before filesystem preparation. Deferred
operation admission and its input channel each have a 1,024-request bound. The
worker keeps receiving controls at capacity and refuses excess admissions with
typed errors, without persisting or later executing them. Cancelled Recovery
reads retire even behind deferred mutations, and shutdown refuses deferred work.
Custom actions expose a waiting state and remain cancellable without starting
their command. Ordinary idle does not poll; only removal-blocked operation
admission checks cancellation and external completion at a bounded interval.

LocalActivity publishes one coalesced host wakeup, including the actual final
work/removal retirement. Iced observes that same hold before last-window exit
and rechecks it on notification. Dropping a browser never releases backend work.
The parent retains removal through the actual request callback or exact helper
teardown after a lost result. Mount requests also reserve accepted-work holds,
preventing a concurrent removal. Backend device attention now retains delivery
responsibility as described below; Iced controls are connected in ADR0093.

The helper-protocol fixture uses the public GIO extension point with real private
files and native hotplug signals. It checks mounted and unmounted discovery,
deduplication, disappearance/reappearance and owner-EOF teardown without a GUI.
Wire-reader tests use actual private Storage files and cover raw root bytes,
preparation failure, malformed/replayed frames and observer-independent updates.
Catalog checks alone do not prove mount/removal execution, encryption dialogs,
Iced focus/navigation, physical unplug/power-off or the complete device requirement.
Record completed results and pending work in the implementation ledger.

Scoped catalog verification passes 68 library parents plus the actual isolated
lazy-init child, four operation-startup tests, all-target Files Clippy, formatting
and a renderer-free local-runtime library check. The actual old helper supplies
the missing-catalog red; the new helper/private provider passes five observation
checks. Existing native Iced operations pass 12 checks and initial headless
residency passes four. Both scoped review axes have zero findings. These results
verify this observation foundation and existing runtime behavior; command/device
UI acceptance and the complete product remain open. Exact evidence is in
[the device work log](../../../.scratch/file-manager/ICED-DEVICES-WIP.md).

Shared admission adds eight public workflow regressions, including real queued
mutations, cancelled processes, failed SQLite result saves, observer-independent
retirement and saturated control admission. The scoped default runs pass 150
parent tests across the library and nine workflow targets, all-target Clippy with
warnings denied, formatting and the renderer-free local-runtime check. The
normal local-runtime dependency graph contains neither Iced nor GStreamer.

The unchanged application passes 25 native checks across headless residency,
same-process reopening with an unanswered decision and queued work, and custom
actions including paged output and process-group cancellation. Two old fixture
Tab counts first targeted the added Recovery control or Undo Cancel; corrected
traversal retains the original clipboard, decision and filesystem assertions.
The Trash workflow's lock-handoff regression now runs in an isolated child after
a raw OperationEngine probe reproduced transient lock inheritance during
parallel child launches. Production locking and retries are unchanged.

Both independent review axes have zero findings on the 18-path implementation
scope. Final fixture traversal and result-documentation changes received primary
review; follow-up independent reviews were unavailable due to the account usage
limit. Those shared-admission checks alone do not establish platform removal,
credential handling, Iced device controls or the full product requirement.

## Helper command path

The existing helper now parses bounded typed command frames and dispatches
asynchronous Mount, safe Drive Stop, Eject and Unmount requests on its owned main
context. Requests revalidate the exact current catalog generation, object and
capability. It supplies no force flag, does not persist requests and publishes a
new observation before completion. The parent sends commands through a bounded
private writer. At that helper checkpoint Iced controls were not wired; this
helper path alone did not complete device support.

MountOperation questions become plain credential/choice observations with a
per-request question sequence. New questions supersede old answers; cancellation
and provider abort retire attention. Cancel and owner EOF await the actual
callback. Credentials are never saved or printed. TrueCrypt fields use GIO's
existing v2_58 API feature, making GLib/GIO 2.58 the minimum for this adapter;
no new Cargo package is added. Blocking-process prompts expose the platform's
message and choices; Files does not kill the listed processes. The actual GIO
signal tests pass alongside the four prepared-observation tests. The default
build, Files all-target Clippy with warnings denied, formatting, maintained
native inventory and dependency inventory pass on the same 633 pinned inputs.
The private helper passes ten command/interaction checks, including actual
credential replies, cancellation, superseded questions, disconnection and owner
EOF. Existing catalog observation passes five checks and headless residency
passes four on the same executable. Provider compilation uses warnings as errors;
no user file or mount is changed. These results verify only this helper path.
Drive stop/eject dispatch compiles but has no provider execution coverage yet.
At that helper checkpoint, parent guard retention, public workflow state and
Iced integration remained open.

## Parent request ownership

DeviceRequests owns one active request and at most 32 recent descriptive results.
Admission reserves LocalActivity before enqueueing, so a fast helper response
cannot race an unrecorded request. Mount holds accepted work; removal owns the
shared removal guard. A 16-frame input queue hands pipe I/O to its own worker.
The reader processes prepared observations before command results, and refuses
invalid response identities or mounted roots. No renderer performs GIO calls or
filesystem preparation.

Cancel clears the visible question but keeps the hold through native teardown.
Answers use the exact current opaque question token; accepted answers retire
that token. Credentials are serialized only into the private pipe, without
Debug/Clone or snapshot/history storage. A refusal explicitly distinguishes an
unaccepted terminal request from a rejected answer to a still-active request.
Only actual completion or a terminal admission refusal retires the normal hold.
Missing completion becomes Uncertain after the exact helper and input writer
retire, while Storage observation loss refuses later filesystem-queue mutations. No success,
navigation or executable restart authority is inferred from a lost reply.

Shutdown closes device admission before filesystem teardown. Active device work
refuses shutdown; a filesystem-queue refusal reopens device admission. Closing a
frontend does not cancel accepted device work. Device results remain separate
from filesystem Undo. Their reported mounted root describes completion; before
enabling automatic UI opening, a backend query must revalidate the current
prepared device/root observation. The frontend then checks only its captured
Window, Tab and navigation intent. The backend query and notification attention
are implemented below. Iced controls are now connected under ADR0093; drive
stop/eject provider execution remains pending.

The queued-action regression subsequently reproduced actual command execution
after successful unmount while the old directory remained present. The managed
Application now supplies prepared Storage to custom actions; workers validate
the original observation after their removal wait. See
[ADR0092](0092-managed-custom-action-storage.md) for this guard and its verification.
It complements the shared scheduling hold and does not grant arbitrary scripts
filesystem transaction or restart authority.

Scoped verification passes 101 default-feature parents and the public runtime
acceptance program. After mechanically equivalent Clippy normalization, all
eight headless library tests, the runtime program, default build, all-target
Clippy with warnings denied, formatting and dependency inventory pass. The
runtime program uses actual private GIO calls, stopped/killed owned helpers,
queued filesystem work and real commands; it checks credential and cancellation
lifetime, stale refusal, preserved bytes, unchanged Undo and uncertainty after
reaping. The same executable passes 25 native checks across helper commands,
catalog observation, headless startup and existing resident reopening. The
resident review capture was inspected. These were scoped backend/host results;
at that parent checkpoint device GUI acceptance remained open. Physical-device
and full-product acceptance remain open. This parent
stage received primary review only; independent reviewers remain unavailable at
the account usage limit.

## Current roots and device attention

DeviceRequests.root requires the exact current catalog token and a prepared
mounted observation. DeviceRequests.mounted_root accepts a completed request ID,
then compares its private mount instance and Storage epoch with the current
device and root. Unrelated catalog revisions preserve this proof; removal,
remount or observer loss invalidates it, including when the old directory still
exists. The proof stays in memory and never enters descriptive results or
serialized state. A missing or expired result requires a new explicit selection.

Before a valid observation changes Storage, the catalog becomes non-navigable.
It returns to Ready only after preparation. EOF or protocol failure also clears
navigation readiness before awaiting helper teardown. Queries take only memory
locks. They describe current prepared observations, not an atomic promise that
a filesystem read cannot subsequently race an unplug; filesystem workers retain
their own Storage validation. Iced must still check its originating Window, Tab
and navigation intent before applying a result.

LocalOperations starts a device-attention host before exposing requests. It
claims its own coalesced backend receiver, leaving the frontend's existing
monitor receiver available. Questions and outcomes reserve notification Work
before the actual device hold retires. Unread attention coalesces to the current
state; already handed deliveries keep exact private receipts through actual
reply or visible refusal. Cancellation or an explicit answer removes unread
obsolete questions. Rejected answers do not duplicate a question notification.

The host hands off at most 17 deliveries at once. Admission checks a threshold
of 1,024 outstanding receipts; the active request can still replace unread
attention or publish a newer question. Pending delivery participates in the
owning application's shared lifetime and removal
gate. Shutdown refuses it. A notification failure is recorded before its hold
is acknowledged, with bounded errors exposed alongside request snapshots. This
work does not poll an idle observer, open a window, answer a question, persist
credentials or grant device-request restart authority.

Nine renderer-free library tests and the public runtime program pass. The
protocol fixture verifies unrelated revisions, remount and failed preparation;
the actual private GIO provider verifies unplug/replug with the old directory
still present. The first test attempt assumed the catalog remained Ready during
preparation; the harness now waits for a prepared observation. This was a test
assumption corrected for the new explicit readiness state.

A separate private-bus fixture passes four checks with no GUI or frontend
subscription: actual mount/removal, a held Notify reply refusing shutdown, a
password question waiting for an exact explicit answer, and all three expected
notifications settling with original bytes preserved and no credential logs.
Default validation passes 103 parents plus the runtime program's nine check
groups, the build, all-target Clippy with warnings denied, formatting and
inventories. The resulting executable passes 25 native helper, catalog,
headless and resident checks; the reopened decision capture was inspected.
All 637 inputs and the application hash match after acceptance. This stage has
primary review only. Iced controls and origin navigation subsequently receive
scoped coverage in [ADR0093](0093-iced-device-controls-and-questions.md).
Physical-device coverage and the product's remaining gates are open. Exact evidence is recorded
in device-attention-verification.json and the device work log.
