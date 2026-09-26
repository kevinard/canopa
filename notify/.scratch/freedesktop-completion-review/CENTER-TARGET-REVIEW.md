# Notification Center target continuation review

User-selected baseline: `df88ecda9f2170ff9a1d1e3f62531882a786761e`. Committed comparison: `git diff df88ecd...HEAD`; cumulative pending comparison: `git diff df88ecd`, including initially untracked ADR 0042. Commits since baseline: `8be0e99`, `9289e34`, `4eaa158`, `cf449ee`, `9d62fcf`, `622201a`, `c170b6d`, `a572593`, `2b8a21b`, `24f66bb`, `9f9f129`, `6260b01`, `aae365b`, `77d3aaf`. This tranche began at `77d3aaf`.

The implementation skill required independent parallel Standards and Spec reviews. Both reviewed the pending tranche and relevant cumulative lifecycle/control/provider interactions without making edits or running builds/tests. Test/check results are recorded separately in [IMPLEMENTATION.md](IMPLEMENTATION.md); static review is not graphical verification.

Both reviewers also rechecked the token-race fixture correction: explicitly poll the public command future to the provider gate, withhold further polling until a bounded competing mutation completes, then release the token and assert stale rejection without sender events. This replaced a paused-clock harness that also fast-forwarded SQLite pool deadlines; no production code changed. Recheck findings remained zero on both axes.

## Standards

Zero scoped findings: no documented-standard violations or actionable baseline smells. Runtime owns lifetime identity and revision-safe commitment; Notify Control exposes typed commands; the provider owns opaque panel encoding. Banner and center interactions reuse lifecycle commitment methods without leaking banner-specific types into center controls. The shared action-key mapping keeps encoding and decoding aligned, while preserving stable panel item IDs.

ADR 0042 explicitly records the superseded center-interaction policy and deployment compatibility boundary, satisfying `docs/agents/domain.md`. Tests exercise existing control/provider/D-Bus interfaces without production-only testing hooks.

Worst outstanding issue: none.

## Spec

Zero scoped findings. Captured controls bind thread ID, revision and daemon lifetime; stale and terminal targets reject before token acquisition, and token-wait races reuse guarded commitment. Lifetime identity handles transient ID reuse without persistence, host changes or capability changes. Stable item IDs and generic opaque-value forwarding are preserved. Missing, mismatched and legacy unguarded item values fail closed; ordinary current-state commands remain separate.

This matches W04's captured revision/generation requirement, W10's replacement-identity requirement and W11's generic protocol requirement. ADR 0042 explicitly supersedes the earlier center-only current-state policy. Tests cover emitted controls, reused sender keys, changed Open/Dismiss meaning, terminal duplicates, restart and token races. Graphical/focus acceptance and broader unfinished work remain deferred.

Worst outstanding issue: none.

Summary: Standards — 0 findings, worst none; Spec — 0 findings, worst none.
