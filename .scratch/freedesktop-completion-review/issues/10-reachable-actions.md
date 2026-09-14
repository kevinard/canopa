# W10: All actions reachable, including overflow/transient banners.

Status: In progress

Specification and acceptance criteria: [PLAN.md](../PLAN.md), work package W10. Execution evidence and remaining work: [IMPLEMENTATION.md](../IMPLEMENTATION.md).

## Comments

2026-09-10: Began implementation under the approved completion plan. An in-progress status does not claim that all package acceptance criteria have passed.

2026-09-11: Continuing from `aae365b` with a runtime-only Notification Center action surface under ADR 0041. Live transient notifications, including banner overflow, must expose all named actions and default activation without entering durable history. Tests use the approved daemon/control, executable/private D-Bus and panel-provider seams. This deliberately supersedes only the earlier transient-center exclusion; compact banner geometry, actual host keyboard/hit-region acceptance, projection overflow/pagination and host-independent expanded banners remain separate.

2026-09-11: Runtime-only center action continuation implemented. Six new public-seam regressions plus strengthened omission/restart fixtures verify live transient content, complete named actions, emitted default/dismiss activation controls, DND/category policy, sticky replacement retention, overflow and display-based expiry, stable ordering and no transient archive. All 86 affected tests and 354 final private-bus workspace tests pass with zero failures or ignored tests. Typecheck, Clippy, formatting, whitespace and Niribar protocol drift checks pass with no new warnings. Independent Spec review found a missing advertised activation affordance; the strengthened executable regression reproduced it, the correction passed, and both reviewers report zero outstanding findings after re-review. See [TRANSIENT-CENTER-REVIEW.md](../TRANSIENT-CENTER-REVIEW.md). Banner geometry, host-independent expanded banners, projection overflow/pagination and actual graphical/keyboard acceptance remain unfinished. No live desktop state was changed.

2026-09-14: Continuing with actual nested-Niri rendering on a headless Sway output, an isolated non-activating session bus and temporary XDG paths. The rich-body/image/progress fixture reproduced clipped body content and invisible action labels in the fixed-height banner. Tests use the approved presentation adapter/renderer-consumer seam with real Iced software layout and input events, followed by the original graphical fixture. Natural card measurement and a bounded, scrollable body preview correct that reproduction; both compact actions and following text now render. The frame handshake also handles same-size replacement and repairs delayed resize delivery. See [graphical evidence](../BANNER-LAYOUT-EVIDENCE.md). Supported scales/small outputs, per-card display acknowledgement, long-label policy, host-independent expansion and full Center keyboard/accessibility acceptance remain unfinished.
