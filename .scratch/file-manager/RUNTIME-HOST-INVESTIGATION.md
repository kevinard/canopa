# Resident host investigation

Status: read-only investigation, 2026-10-18. No resident host, activation protocol
or new public executable is implemented by these notes. ADR0074/0076 and the full
product brief govern the work. Prepared during frozen clipboard validation;
this scratch investigation is not a production input.

## Existing reusable behavior

- `operation_jobs.rs` owns durable admission, serial mutation/transfer/Undo
  dispatch, cancellation, progress, generation-bound decisions and result saves.
  Its snapshot describes holds independently of Iced. Public tests run with
  `--no-default-features` against real private filesystem state.
- `operation_jobs/clipboard.rs` now prepares owned selected-entry observations,
  original Storage and retention before publication, then owns Cut claims and
  actual durable partial completion. Native publication tokens remain in the
  Wayland transport adapter.
- `actions.rs` and `process.rs` own argument preparation, execution, cancellation
  and bounded output. Browsing/search/preview/configuration/storage domain
  modules are independent of Iced.
- `OperationJobs::shutdown` refuses shutdown while admitted work or unsaved
  results need protection and replies after releasing the history writer.
  Dropping its final handle is a stop/cancellation request, not detach semantics.

## Host work still coupled to Iced

1. `iced_desktop/mod.rs::run` loads application configuration/preferences and
   initializes `iced::daemon` before starting volume observation and operations.
   The executable is gated behind Cargo's `desktop` feature. There is no current
   headless command path; zero Iced windows alone does not establish one.
2. `iced_desktop/file_operations.rs::State` owns the OperationJobs handle and
   local volume Monitor. Its startup sequences asynchronous initial observation
   before operation admission. A future host must retain both when every frontend
   detaches, preserving observer-loss refusal and original Storage semantics.
3. `OperationJobs::take_notices` transfers one coalesced receiver. It is suitable
   for its current one application observer, not a multi-client event protocol.
   A host can own that receiver and publish bounded snapshots to attached
   frontends; do not let new frontends steal it or enqueue unlimited snapshots.
4. The historical Iced custom-action queue described here is now extracted under
   ADR0078. `action_jobs::ActionJobs` owns admission, FIFO dispatch, cancellation,
   completed-output trimming and compact completion handoff without observer
   participation. The adapter retains output editors, pagination, focus and
   native notification-delivery bookkeeping. That remaining attention lifetime
   still needs ownership outside Iced before claiming a complete headless host.
5. `iced_desktop/configuration_ui.rs` owns the ordered configuration worker and
   polling lifetime around an existing core ConfigurationRuntime. Notification
   delivery/bookkeeping and application exit holds also live in Iced. Moving
   process ownership must account for these explicitly; a forwarding facade does
   not remove the caller's sequencing burden.
6. Browser/search/preview sessions hold Iced window identities in their delivery
   tickets. Their underlying models are reusable, but worker ownership and stale
   result routing need application session identities where they cross the host
   seam. Closing a Browser may cancel its read/search/preview work; it must not
   cancel admitted file operations or explicitly accepted custom actions.

## Decisions still needed for implementation

Choose process layout and local activation transport together. A separate core
host can isolate accepted filesystem work from a frontend crash; an Iced process
with zero windows does not supply that property. Preserve a single history owner,
byte-exact paths and typed generation-bound decisions whatever transport is used.
Do not serialize opaque live handles into guessed replay permission.

A normal launch must connect to the established owner, request new windows and
return a visible error on an incompatible protocol or failed launch. Concurrent
cold starts must elect one owner before accepting mutations. Activation cannot
create a second writer when an older installed runtime is still active. The
shared updater/launcher decision in root ADR0017 requires an explicit integration
update before packaging activation or resident service changes.

Resident/headless acceptance must distinguish frontend detach from runtime
shutdown or crash. No implicit answer is supplied by a lost frontend. The same
pending question and actual operation progress must reappear on reattachment.
Orderly shutdown cannot silently discard unsaved results. Runtime restart still
needs existing journal/source/media reconciliation gates; running without windows
is not evidence of safe saved replay.

Use the established public operation, action and storage seams for core behavior,
and private desktop sessions for actual activation, closing all windows during
work/questions and reconnecting to the same queue. Measure idle CPU/memory and
warm opening latency. Do not add always-running preview caches or closed-browser
watchers as a substitute for measuring the startup path.

## Display-free activation constraint (read-only, 2026-10-19)

During immutable configuration-runtime validation, the repository fork shows
`vendor/winit/src/platform_impl/linux/mod.rs:738` selecting the backend from
WAYLAND_DISPLAY/WAYLAND_SOCKET/DISPLAY, and the Wayland event-loop constructor
(`wayland/event_loop/mod.rs:90`) calls Connection::connect_to_env. Public
EventLoopBuilderExtWayland only exposes forced backend and any-thread selection;
there is no supplied connection/display-name interface. No vendor change made.

The single-process/lazy-Iced option in NEXT-HOST-SLICE is still a proposal. If
the owner starts with no display environment and has already started backend
threads, a later display-bearing activation cannot be implemented by casually
mutating the process environment. Evaluate that constraint before accepting the
process layout: retain a session display connection from startup for ordinary
residency, add a justified explicit connection seam with vendor provenance, or
use a separate frontend process and typed local transport. These have different
acceptance/complexity costs. Do not silently weaken the no-GUI initialization
acceptance target or claim separate-process crash isolation from zero windows.

Volume/operation startup and notification ownership can still be extracted at
the existing platform/core seams without precommitting to process layout.
