# 0082: Backend-owned custom-action startup and completion attention

Status: scoped verification, 2026-10-19. Continues ADR0074/0076/0078/0081.

LocalActions owns command startup and completion attention before Iced starts.
It contains the existing ActionJobs queue and the session notification adapter.
The owning handle lives outside the frontend; frontend clones, snapshots and
coalesced notices are optional observers. Dropping them cannot gate accepted
work or completion delivery. The final owner still requests stopping the runtime.

The background-actions Cargo feature builds this workflow without Iced. Its
Tokio event loop waits on queue changes, transport replies, shutdown and owner
drop; it does not poll idle state. The existing standard notification transport
now lives outside iced_desktop and starts before GUI initialization. Filesystem
attention still has its current Iced coordination and shares that transport.
Notify retains desktop notification policy and presentation.

LocalActions exposes captured Submit, Cancel, snapshots and orderly Shutdown.
It does not expose the underlying receipt-draining interface. The platform owner
takes completion responsibility independently of a GUI. A shared attention lock
covers the handoff and snapshot reads, so the queue's completion hold cannot
disappear before the platform delivery hold exists. Shutdown runs on the owner
and refuses pending delivery; while checking core shutdown it leaves unhanded
receipts in the core so their hold cannot cross that check.

Keep the existing 4 command workers, 1024 admission/receipt limit, 100 output rows,
16-slot transport and 2-second notification timeout. At most 17 notification
reply futures are retained by this adapter (one active transport request and
sixteen queued). Busy/unavailable delivery is observable, with errors bounded by
retained rows and one latest error for an already expired row. No silent success,
unbounded pending Task set or fabricated file Undo is introduced.

Iced attaches to the existing owner, submits explicit requests, renders output
and notification errors, and consults the backend's current hold before exiting.
It no longer creates ActionJobs lazily, drains completion receipts or schedules
their delivery. Widget drafts, output pages, clipboard output copying and native
window state remain presentation responsibilities.

The public action/platform seams under ADR0001 use actual private scripts/files
and the standard D-Bus interface. A no-GUI test drops frontend/notices and checks
command execution with a known transport-startup refusal. A maintained private
bus test builds first, then runs without DISPLAY, Wayland or a compositor. Its
notification receiver deliberately withholds a real Notify reply. All twenty
commands must finish with exact file bytes, while pending deliveries remain
bounded and busy failures remain visible. Shutdown must refuse during the actual
reply hold; releasing replies must settle delivery without new failures and
permit orderly shutdown. The external receiver verifies the exact delivered-title
set independently of the runtime snapshot.

Source07 passes five focused no-GUI tests (four unchanged ActionJobs tests and
one LocalActions workflow), the maintained no-GUI private-bus case (four checks),
47 Iced library tests, default all-target Clippy and formatting. Clippy reports
three existing warning groups. The private-bus burst completes twenty commands,
delivers seventeen notices and reports three busy failures. Its build contains
only the background-actions Files feature and no Iced compilation artifacts.

Normal binary ab3caff0 passes three native workflows and 42 checks: Iced actions
in both themes and light-theme file operations. They cover editing, captured
arguments, bounded output, cancellation, attention and lifetime after Browser
closure, plus shared transport use by file operations. The operation log is
empty; each action log has only four expected unavailable GIO remote-monitor
messages on the isolated bus. Representative result, output and background-work
captures were inspected. Standards and Spec reviews each report zero scoped
findings. Final source08 changes documentation only; scoped evidence is in
`.scratch/file-manager/action-host-verification.json`.

This does not implement a resident CLI, primary-instance activation, process
isolation, filesystem attention extraction, durable command replay or any whole
product/workspace/release gate. The independent startup timeout remains open.
