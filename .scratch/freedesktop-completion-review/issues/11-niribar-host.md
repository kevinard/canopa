# W11: Port Canopa protocol v3 to the confirmed Niribar host; preserve existing framed-v1 providers.

Status: complete for the selected static Notifications 1.3 profile

Specification and acceptance criteria: [PLAN.md](../PLAN.md), work package W11. Execution evidence and remaining work: [IMPLEMENTATION.md](../IMPLEMENTATION.md).

## Comments

2026-09-10: Began implementation under the approved completion plan. An in-progress status does not claim that all package acceptance criteria have passed.

2026-09-14: Opened the unchanged Canopa-v3 Niribar host (`b5d1b54`) on a nested Niri compositor inside headless Sway with private state and a non-activating D-Bus session. The provider panel displays Canopa content; screenshots and environment details are in [BANNER-LAYOUT-EVIDENCE.md](../BANNER-LAYOUT-EVIDENCE.md). Complete Center rich-content/action scrolling, reliable graphical keyboard selection and full-session accessibility remain unverified. This smoke check does not enable `body-images` or close W11.

2026-09-14: The [keyboard continuation](../CENTER-KEYBOARD-EVIDENCE.md) corrects long selected labels hiding panel content and old pointer presses activating replacement controls. Three real-widget regressions and all 1,145 host tests pass. The rebuilt host passes paced fourth/default action, replacement reset and Escape checks; hidden transient actions remain usable behind three critical banners without starting their finite timeout. Independent review clears the stale-press finding. The pinned Iced/layer-shell stack has no implemented accessibility-tree bridge; full screen-reader semantics, full keyboard reading/rapid-key behavior, rich-content/permission/scale checks and host recovery remain open. W11 stays in progress and `body-images` stays unadvertised.

2026-09-14: The [window-lifetime follow-up](../CENTER-WINDOW-EVIDENCE.md) isolates rapid-key loss to panel recreation. Canopa navigation, replacement and failure content now retain their window; stale filtering and selection reset remain intact. Six new application regressions and all 1,151 host tests pass. Forty nested-Niri rapid/paced action trials keep one window and invoke the correct fourth action; replacement reset and Escape pass. A review-found v1 stale-press regression was avoided by retaining v1's existing lifecycle. Held-key stress, overflowing-content keyboard reading and full W11/accessibility acceptance remain open.

2026-09-21 maintenance: Status reconciled with the final [implementation tracker](../IMPLEMENTATION.md) and [release verification](../../../docs/notification-release-verification.md). Earlier dated comments retain historical evidence. Operational rollout, additional failure verification, and hosted CI are tracked separately in [project completion](../../project-completion/PRD.md); hosted execution remains open until a repository and runner exist.
