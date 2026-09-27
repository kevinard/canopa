# Executable D-Bus contract continuation review

User-selected baseline: `df88ecda9f2170ff9a1d1e3f62531882a786761e`. Committed comparison: `git diff df88ecd...HEAD`; cumulative pending comparison: `git diff df88ecd`, including initially untracked `crates/canopa-notify/tests/dbus_contract.rs`. Commits since baseline: `8be0e99`, `9289e34`, `4eaa158`, `cf449ee`, `9d62fcf`, `622201a`, `c170b6d`, `a572593`, `2b8a21b`, `24f66bb`, `9f9f129`, `6260b01`, `aae365b`, `77d3aaf`, `60d733f`. This tranche began at `60d733f`.

The implementation skill required independent parallel Standards and Spec reviews. Both reviewed the pending tranche and relevant cumulative adapter/runtime interactions without editing files or running builds/tests. Verification results are recorded separately in [IMPLEMENTATION.md](IMPLEMENTATION.md); neither review certifies graphical acceptance or completion of the overall plan.

## Standards

Zero scoped findings. The injected-header workaround is documented and contract-protected without leaking binding details into core modules. The executable fixture establishes child cleanup before readiness assertions, isolates persistent/configuration state, and exercises approved public seams. Tests preserve the Sender Notification ID/Internal Notification ID boundary and strengthen global signal-order assertions. No actionable baseline smells or documented-standard violations were found in this tranche and its relevant cumulative interactions.

Worst outstanding issue: none.

## Spec

Zero scoped findings. W01 asks that method introspection, server information, capabilities, error replies and event payloads be checked at the D-Bus interface. The six executable tests and strengthened activation test substantiate this tranche. Exact argument boundaries are checked in introspection; independent `dbus-send` decoding prevents the Rust serialization stack from masking a struct instead of four top-level server-information strings. Real error envelopes, opaque action keys, resident behavior, sender IDs and ordered signals are exercised through approved public seams.

Injecting the header enables generated empty-argument validation without changing discovery signatures or valid replies. Documentation distinguishes generated errors, allocation/admission policy and empty-string error conventions from normative requirements. No scoped missing requirement, scope creep or incorrectly implemented requirement was found. Graphical acceptance and broader W01 completion remain open.

Worst outstanding issue: none.

Summary: Standards — 0 findings, worst none; Spec — 0 findings, worst none.
