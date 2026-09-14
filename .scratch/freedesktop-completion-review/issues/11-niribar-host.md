# W11: Port Canopa protocol v3 to the confirmed Niribar host; preserve existing framed-v1 providers.

Status: In progress

Specification and acceptance criteria: [PLAN.md](../PLAN.md), work package W11. Execution evidence and remaining work: [IMPLEMENTATION.md](../IMPLEMENTATION.md).

## Comments

2026-09-10: Began implementation under the approved completion plan. An in-progress status does not claim that all package acceptance criteria have passed.

2026-09-14: Opened the unchanged Canopa-v3 Niribar host (`b5d1b54`) on a nested Niri compositor inside headless Sway with private state and a non-activating D-Bus session. The provider panel displays Canopa content; screenshots and environment details are in [BANNER-LAYOUT-EVIDENCE.md](../BANNER-LAYOUT-EVIDENCE.md). Complete Center rich-content/action scrolling, reliable graphical keyboard selection and full-session accessibility remain unverified. This smoke check does not enable `body-images` or close W11.

2026-09-14: The [keyboard continuation](../CENTER-KEYBOARD-EVIDENCE.md) corrects long selected labels hiding panel content and old pointer presses activating replacement controls. Three real-widget regressions and all 1,145 host tests pass. The rebuilt host passes paced fourth/default action, replacement reset and Escape checks; hidden transient actions remain usable behind three critical banners without starting their finite timeout. Independent review clears the stale-press finding. The pinned Iced/layer-shell stack has no implemented accessibility-tree bridge; full screen-reader semantics, full keyboard reading/rapid-key behavior, rich-content/permission/scale checks and host recovery remain open. W11 stays in progress and `body-images` stays unadvertised.
