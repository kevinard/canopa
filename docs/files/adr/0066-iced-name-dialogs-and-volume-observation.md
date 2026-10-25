# 0066: Iced names, shared Operations and volume observation

Status: implementation in progress, 2026-10-18. This continues ADR0065 and the
full product brief; it does not replace remaining operation/continuation gates.

Use ordinary source-bound Iced transient windows for New Folder and Rename.
Keep a single basename draft, the original raw path and resolved Browser binding
until durable admission replies. Show the destination folder in the dialog.
Validation keeps a stable notice slot and the native input's editing state.
Confirming an unchanged lossy display name closes without modifying its raw path.

The shared Operations window contains filesystem jobs alongside custom commands.
Filesystem admission, questions, cancellation and Save Retry use the core worker.
Undo always captures a concrete current history ID. Show the engine's complete
conflict facts and capability-dependent choices, with Cancel as the default.
Only present an automatic question when its source or Operations is focused;
otherwise leave an explicit Review action. Closing a source window hides its
question without discarding the independent operation.

Completion refreshes affected browsing snapshots and preserves independent tab
state. Reveal a new name only if its source navigation and selection still match.
Compare captured resolved directory bindings as well as raw paths so another
view through an alias refreshes too. A completion during directory loading must
preserve that navigation. Mark its pending snapshot stale and rescan the eventual
destination after loading, without converting Visit into a different location.
Keep drafts until acceptance; accepted work survives source closure and holds the
application through active work, unanswered questions and unsaved results.

Distinguish a failed start checkpoint from an unsaved stopped result. Retry of
the former still executes its original admitted request and keeps its source
tracking until actual completion. Report attention once while waiting to save;
refresh/reveal only after work stops. Retry of a result save never executes its
original request again. Both save failures hold the application and expose Retry
Save. If the compositor forcibly closes the last Operations window while a save
is outstanding, reopen it so the held process remains reachable.

Live storage observations reach the same core Storage instance that admits
requests. GIO's VolumeMonitor requires its main thread and the default context,
so a dedicated helper process hosts its event loop. See the
[GIO contract](https://docs.gtk.org/gio/class.VolumeMonitor.html). These are
platform services, with no GUI widgets. The helper executes the running image
through `/proc/self/exe`, preserving protocol compatibility across installation
updates. A hidden internal mode runs before configuration or Iced initialization.

The helper emits versioned, monotonic observations over a private bounded pipe.
Frames retain raw paths, stable identities, mount-object incarnations and signal
revisions, with limits of 1024 roots and 4 MiB. The parent prepares the initial
snapshot off the UI thread before admitting operations. EOF stops the helper's
main loop and its reader reaps that exact child. Losing the observer invalidates
all pending placement checks, including paths outside known roots. Undo checks
placement again after a question before acting on the answer. Original UUID/epoch
and resolved browsing requirements remain authoritative.

Native verification uses the repository's isolated Niri harness, a private HOME,
actual temporary files, both themes and narrow/wide windows. It covers selected
initial names, validation correction, raw-name no-op, real Rename/Create/Undo,
collisions, independent windows and Operations. The pre-change binary times out
waiting for the Iced New Folder dialog, establishing the initial failing case.
Expanded native checks also queue work behind a live Undo question, close every
Browser, restore its question through Operations, deliver private GIO unplug
signals and terminate only the exact application-owned helper through a pidfd.
Both themes preserve later files and refuse the queued mutation after volume or
observer loss. Controller regressions cover independent navigation and aliases;
a real database write failure verifies start-checkpoint Retry without losing its
source tracking.

Device browsing/actions, full external exchange, exact restart continuation,
source owners, descriptor authority, media and full workspace/release acceptance
remain required. The initial mount observer is not completion of device support.
