# Renderer-readiness continuation review

User-selected baseline: `df88ecda9f2170ff9a1d1e3f62531882a786761e`. Committed comparison: `git diff df88ecd...HEAD`; cumulative pending comparison: `git diff df88ecd`, including initially untracked `crates/canopa-notify/tests/renderer_startup.rs` and ADR 0043. Commits since baseline: `8be0e99`, `9289e34`, `4eaa158`, `cf449ee`, `9d62fcf`, `622201a`, `c170b6d`, `a572593`, `2b8a21b`, `24f66bb`, `9f9f129`, `6260b01`, `aae365b`, `77d3aaf`, `60d733f`, `6397b9e`, `1bf5698`. This tranche began at `1bf5698`.

The implementation skill required independent parallel Standards and Spec reviews. Both reviewed the pending tranche and its relevant cumulative presentation/startup interactions without editing files or running builds/tests. Verification results are recorded separately in [IMPLEMENTATION.md](IMPLEMENTATION.md); neither review certifies graphical acceptance or completion of W02/W05/the overall plan.

## Standards

Zero scoped findings: no documented-standard breaches or actionable baseline smells. The changes preserve ADR 0030's commitment boundary and implement ADR 0043's readiness, terminal health and cancellation policy behind the presentation adapter. Closing the send gate before releasing callbacks avoids reentrant retirement locking; late readiness cannot replace `Stopped`.

Tests cover observable history preservation, idle Control status, continued receipt/Center behavior, cancellation and renderer shutdown. The executable fixture uses temporary state/config/socket paths and an unresponsive local Wayland endpoint; its canonical D-Bus name relies on the documented private-bus runner.

Worst outstanding issue: none.

## Spec

One P2 documentation finding was corrected and independently rechecked. The initial wording promised degraded-service survival immediately after first readiness, but startup can still fail at the second readiness check after publishing an import. ADR 0043 and service-mode documentation now distinguish failure before initial readiness, later startup failure after import publication, and degraded service after startup completes.

Zero remaining scoped findings: no missing scoped requirements, scope creep or incorrect implementations. Vendored Iced/layer-shell source confirms readiness is emitted after window, renderer, surface creation and configuration. Timeout/cancellation terminality, idle health and transport retirement match the scope. Automatic replacement, complete shutdown draining and actual graphical acceptance remain explicitly deferred.

Worst outstanding issue: none.

Summary: Standards — 0 findings, worst none; Spec — 0 remaining findings (1 P2 documentation correction), worst outstanding none.
