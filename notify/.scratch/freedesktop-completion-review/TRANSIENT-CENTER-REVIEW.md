# Transient-center continuation review

User-selected baseline: `df88ecda9f2170ff9a1d1e3f62531882a786761e`. Committed comparison: `git diff df88ecd...HEAD`; cumulative pending comparison: `git diff df88ecd`, including initially untracked provider fixtures and ADR 0041. Commits since baseline: `8be0e99`, `9289e34`, `4eaa158`, `cf449ee`, `9d62fcf`, `622201a`, `c170b6d`, `a572593`, `2b8a21b`, `24f66bb`, `9f9f129`, `6260b01`, `aae365b`. This tranche began at `aae365b`.

The implementation skill required independent parallel Standards and Spec reviews. Both inspected this tranche and cumulative lifecycle/control/provider interactions, then independently rechecked the default-activation correction. Reviewers made no edits and ran no builds or tests. Actual graphical/keyboard acceptance, banner geometry, host-independent expanded banners, projection overflow/pagination and broader recovery remain unfinished.

## Standards

Zero scoped findings, including the correction re-review. Lifecycle remains the owner of transient threads; Runtime derives center content; the adapter owns retention labels and item-activation controls. The projection's `has_default_action` fact serves the concrete label decision without exposing action parsing to the adapter. Named sender action keys remain opaque and separate from adapter control keys. Tests use the existing approved interfaces without production test hooks. No actionable baseline smells or conflicts with the documented ownership rules were found.

Worst outstanding issue: none.

## Spec

One P2 finding was identified and corrected. The first provider fixture invented an activation event that the outgoing frame never advertised. Niribar renders and authorizes only advertised item actions; this was a static contract gap, not merely missing graphical evidence. Active items now advertise `Open notification` for default activation or `Dismiss notification` otherwise. The strengthened fixture reproduced the missing control and passed after the correction; it invokes only the control emitted by the provider. Another executable fixture verifies dismissal of transient/retained items and absence of controls on inactive history.

Independent re-review confirmed that the existing host renders, registers for keyboard selection, and whitelists the emitted generic action. No host or shared-protocol source change is required. Live transient content, sticky retention, terminal/restart disappearance, policy and timeout semantics match ADR 0041. No remaining scoped requirement gap, scope creep or incorrect implementation was found. Final verification evidence is in [IMPLEMENTATION.md](IMPLEMENTATION.md).

Worst outstanding issue: none. The original P2 is resolved.

Summary: Standards — 0 findings, worst none; Spec — 1 P2 corrected, 0 outstanding, worst outstanding none.
