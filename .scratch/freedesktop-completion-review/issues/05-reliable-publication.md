# W05: Reliable ordered sender events and recoverable current-state presentation.

Status: In progress

Specification and acceptance criteria: [PLAN.md](../PLAN.md), work package W05. Execution evidence and remaining work: [IMPLEMENTATION.md](../IMPLEMENTATION.md).

## Comments

2026-09-10: Began implementation under the approved completion plan. An in-progress status does not claim that all package acceptance criteria have passed.

2026-09-10 continuation: Reproduced renderer FIFO overflow and unbounded undisplayed acknowledgements. Implement a latest-state stream, bounded pending frame acknowledgement and renderer-lifetime cleanup. Preserve acknowledgements from already queued frames, but never acknowledge skipped undisplayed states. Tests use the existing presentation adapter/renderer-consumer seam. Startup readiness, proactive health, restart recovery and full shutdown/transport-failure verification remain separate unfinished criteria.

2026-09-11 continuation from `1bf5698`: Implement renderer event-loop readiness before history recovery/import, bounded startup waits and idle renderer-exit health at the approved Daemon/Control, executable and presentation adapter/renderer-consumer seams. A first processed window redraw is the readiness milestone, not a proof of visible compositor output. Preserve committed sender lifecycles and the Notification Center after renderer failure, report degraded health, and require an explicit daemon restart to restore banners for now. Cover delayed, failed, timed-out/cancelled startup and renderer retirement without new receipt activity. Automatic renderer restart, complete shutdown draining and actual graphical acceptance remain separate. Review baseline stays `df88ecd`.

2026-09-11 verification: ADR 0043 implements bounded first-redraw readiness, idle terminal health and startup/last-owner retirement behind the presenter. Seven new tests cover Daemon/Control, renderer-consumer and executable/nonresponsive-Wayland behavior. All 22 focused and 377 full-workspace tests pass; typecheck, Clippy, formatting, whitespace and Niribar protocol-snapshot checks pass with existing warnings. Independent Standards/Spec reviews have zero remaining scoped findings after correcting the documented startup/import boundary; see [RENDERER-READINESS-REVIEW.md](../RENDERER-READINESS-REVIEW.md). W05 remains in progress for automatic recovery/replay, full shutdown/signal-transport failure evidence and actual graphical acceptance.
