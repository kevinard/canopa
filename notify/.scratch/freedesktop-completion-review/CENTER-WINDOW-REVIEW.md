# Notification Center window-lifetime review

Date: 2026-09-14. User-selected Notify baseline: `df88ecda9f2170ff9a1d1e3f62531882a786761e`; host-port baseline: `d7870c2d20cff5d0f84da03296e15bf65115cc90`.

Historical commands: `git diff df88ecd...HEAD` in Notify and `git diff d7870c2d20cff5d0f84da03296e15bf65115cc90...HEAD` in Niribar, with their corresponding `git log <baseline>..HEAD --oneline` inventories. The bounded continuation starts at Notify `ad4e008` and Niribar `bdda852`; reviewers additionally inspected the working changes and new application test file. Earlier unrelated continuations are not re-reviewed here.

Scope: PLAN W11, host issue 11, accessibility issue 35, existing foreground/session domain rules and ADRs 0008/0011. The two independent review axes ran in parallel. This is not overall freedesktop or accessibility sign-off.

## Standards

Zero documented-standard breaches and zero actionable heuristic smells. External Panel Session retains content ownership, Foreground validates active Panel identity and retains physical window ownership, and Panel Runtime remains private behind Foreground. No public protocol or notification-domain interpretation was introduced.

## Spec

One initial P1: the broad update path preserved framed-v1 button state across replacement, allowing a pointer press begun on old content to complete against a new button. Canopa has generation-owned action widgets, but v1 does not. The fix is scoped to Canopa; v1 replacement, failure and local echo retain their previous window-recreation behavior. New compatibility tests guard that lifecycle. Independent source recheck reports zero outstanding findings.

Both final evidence/document rechecks report zero outstanding findings. Reviewers independently checked the scripts, forty fourth-action signals, one panel-opening log, all four captures, six focused regressions and the 1,151-test suite totals. [Evidence](CENTER-WINDOW-EVIDENCE.md) records both failing regressions and final verification. Notify runtime is unchanged, so its workspace suite was not rerun. The verified Niribar correction is committed as `05e6e5f`.

Summary: Standards **0**; scoped Spec **0 outstanding** after the P1 compatibility correction. Full W11 remains open.
